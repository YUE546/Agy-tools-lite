//! Antigravity's Secret Service contract. No shell commands or token-bearing arguments.
use secret_service::{Collection, EncryptionType, SecretService};
use std::{collections::HashMap, future::Future, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(10);

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("service", "gemini"), ("username", "antigravity")])
}

fn describe(error: secret_service::Error) -> String {
    match error {
        secret_service::Error::Locked => "Linux credential store is locked; unlock your desktop keyring and retry.".into(),
        secret_service::Error::Prompt | secret_service::Error::PromptDisconnected => "Linux keyring unlock was cancelled or unavailable; unlock your desktop keyring and retry.".into(),
        secret_service::Error::NoResult => "No Antigravity credential found in the Linux keyring.".into(),
        _ => format!("Linux Secret Service unavailable: {error}. Start a desktop Secret Service (GNOME Keyring or compatible KWallet) in your user D-Bus session."),
    }
}

async fn bounded<T>(
    future: impl Future<Output = Result<T, String>>,
    timeout: Duration,
) -> Result<T, String> {
    tokio::time::timeout(timeout, future).await.map_err(|_| {
        "Linux credential operation timed out; check your session D-Bus and unlock the keyring."
            .to_string()
    })?
}

// Migration readers are synchronous and can also be called inside Tokio. A dedicated
// runtime avoids nested block_on calls; all D-Bus work below has a deadline.
fn run<T: Send + 'static>(
    future: impl Future<Output = Result<T, String>> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("Cannot start Linux credential runtime: {error}"))?;
        runtime.block_on(future)
    })
    .join()
    .map_err(|_| "Linux credential worker failed.".to_string())?
}

async fn connect() -> Result<SecretService<'static>, String> {
    bounded(
        async {
            SecretService::connect(EncryptionType::Dh)
                .await
                .map_err(describe)
        },
        TIMEOUT,
    )
    .await
}

async fn collections<'a>(service: &'a SecretService<'_>) -> Result<Vec<Collection<'a>>, String> {
    bounded(async {
        // go-keyring addresses /collection/login directly, not the default alias.
        let mut collections: Vec<_> = service.get_all_collections().await.map_err(describe)?
            .into_iter().filter(|collection| collection.collection_path.as_str().ends_with("/login")).collect();
        if let Ok(default) = service.get_default_collection().await {
            if !collections.iter().any(|collection| collection.collection_path == default.collection_path) {
                collections.push(default);
            }
        }
        if collections.is_empty() {
            return Err("No persistent login/default keyring exists. Set up and unlock your desktop keyring before switching accounts.".into());
        }
        Ok(collections)
    }, TIMEOUT).await
}

async fn snapshot(collection: &Collection<'_>) -> Result<Option<Vec<u8>>, String> {
    bounded(async {
        collection.unlock().await.map_err(describe)?;
        let items = collection.search_items(attributes()).await.map_err(describe)?;
        if items.len() > 1 {
            return Err("Multiple Antigravity credentials exist in one keyring; resolve the duplicate entries before switching.".into());
        }
        if let Some(item) = items.first() {
            item.unlock().await.map_err(describe)?;
            Ok(Some(item.get_secret().await.map_err(describe)?))
        } else {
            Ok(None)
        }
    }, TIMEOUT).await
}

async fn replace(collection: &Collection<'_>, payload: &[u8]) -> Result<(), String> {
    bounded(
        async {
            let item = collection
                .create_item(
                    "Password for 'antigravity' on 'gemini'",
                    attributes(),
                    payload,
                    true,
                    "text/plain",
                )
                .await
                .map_err(describe)?;
            if item.get_secret().await.map_err(describe)? != payload {
                return Err(
                    "Linux keyring credential readback did not match the requested account.".into(),
                );
            }
            Ok(())
        },
        TIMEOUT,
    )
    .await
}

async fn recover(
    collections: &[Collection<'_>],
    previous: &[Option<Vec<u8>>],
    last: usize,
    error: String,
) -> String {
    let mut rollback_failed = false;
    for index in (0..=last).rev() {
        let collection = &collections[index];
        let result = if let Some(old) = &previous[index] {
            replace(collection, old).await
        } else {
            bounded(
                async {
                    for item in collection
                        .search_items(attributes())
                        .await
                        .map_err(describe)?
                    {
                        item.delete().await.map_err(describe)?;
                    }
                    Ok(())
                },
                TIMEOUT,
            )
            .await
        };
        rollback_failed |= result.is_err();
    }
    if rollback_failed {
        format!(
            "{error} Credential recovery was incomplete; check the active account before retrying."
        )
    } else {
        format!("{error} Previous credentials were restored.")
    }
}

pub fn write(payload: &str) -> Result<(), String> {
    write_with_commit(payload, || Ok(()))
}

pub fn write_with_commit(
    payload: &str,
    commit: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let payload = payload.as_bytes().to_vec();
    run(async move {
        let service = connect().await?;
        let collections = collections(&service).await?;
        let mut previous = Vec::new();
        for collection in &collections {
            previous.push(snapshot(collection).await?);
        }
        for (index, collection) in collections.iter().enumerate() {
            if let Err(error) = replace(collection, &payload).await {
                return Err(recover(&collections, &previous, index, error).await);
            }
        }
        if let Err(error) = commit() {
            return Err(recover(&collections, &previous, collections.len() - 1, error).await);
        }
        Ok(())
    })
}

pub fn read() -> Result<String, String> {
    run(async {
        let service = connect().await?;
        for collection in collections(&service).await? {
            if let Some(secret) = snapshot(&collection).await? {
                return String::from_utf8(secret)
                    .map_err(|_| "Linux credential payload is not valid UTF-8.".into());
            }
        }
        Err("No Antigravity credential found in the Linux keyring.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_service_has_actionable_error() {
        assert!(describe(secret_service::Error::Unavailable).contains("D-Bus"));
        assert!(describe(secret_service::Error::Locked).contains("unlock"));
    }

    #[test]
    fn credential_deadline_cancels_a_stalled_operation() {
        let result = run(async {
            bounded(
                std::future::pending::<Result<(), String>>(),
                Duration::from_millis(25),
            )
            .await
        });
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[test]
    #[ignore = "requires a disposable D-Bus/keyring session"]
    fn isolated_secret_service_roundtrip() {
        assert_eq!(
            std::env::var("ANTIGRAVITY_TEST_SECRET_SERVICE").as_deref(),
            Ok("isolated")
        );
        run(async {
            let service = connect().await?;
            assert!(!collections(&service).await?.is_empty());
            Ok(())
        })
        .unwrap();
        let old = r#"{"token":{"refresh_token":"fixture-old"}}"#;
        let new = r#"{"token":{"refresh_token":"fixture-new"}}"#;
        write(old).unwrap();
        assert_eq!(read().unwrap(), old);
        // Make default a distinct collection; both readers must see the new token.
        run(async {
            let service = connect().await?;
            let other = service
                .get_all_collections()
                .await
                .map_err(describe)?
                .into_iter()
                .find(|collection| collection.collection_path.as_str().ends_with("/other"))
                .ok_or("isolated other collection fixture is missing")?;
            let result = std::process::Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    "org.freedesktop.secrets",
                    "--object-path",
                    "/org/freedesktop/secrets",
                    "--method",
                    "org.freedesktop.Secret.Service.SetAlias",
                    "default",
                    other.collection_path.as_str(),
                ])
                .output()
                .map_err(|error| error.to_string())?;
            assert!(result.status.success(), "fixture default alias must be set");
            Ok(())
        })
        .unwrap();
        write(new).unwrap();
        assert_eq!(read().unwrap(), new);
        let failure = write_with_commit(old, || Err("fixture CLI write failed".into()));
        assert!(failure
            .unwrap_err()
            .contains("Previous credentials were restored"));
        assert_eq!(read().unwrap(), new);
        run(async move {
            let service = connect().await?;
            let collections = collections(&service).await?;
            assert_eq!(
                collections.len(),
                2,
                "login and default must be separate in this fixture"
            );
            for collection in collections {
                assert_eq!(snapshot(&collection).await?.unwrap(), new.as_bytes());
            }
            Ok(())
        })
        .unwrap();
    }
}
