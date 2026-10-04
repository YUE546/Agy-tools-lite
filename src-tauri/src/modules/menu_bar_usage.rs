//! Today's local usage and API-equivalent estimate. The menu only reads the
//! existing public pricing cache, so opening it never waits for a price fetch.
use super::{api_pricing::{ApiPricing, ApiPricingSnapshot}, native_token_stats::{LocalTokenModel, LocalTokenTotals, LocalTokenUsageSummary}};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MenuBarUsage {
    pub today: LocalTokenTotals,
    pub estimated_usd: Option<f64>,
    pub unpriced_models: usize,
    pub pricing_stale: bool,
    pub incomplete: bool,
}

pub fn project(summary: &LocalTokenUsageSummary, pricing: Option<&ApiPricingSnapshot>) -> MenuBarUsage {
    let (mut estimated_usd, unpriced_models) = estimate(&summary.by_model_today, pricing.map(|pricing| pricing.prices.as_slice()).unwrap_or_default());
    if summary.today.total_tokens > 0 && summary.by_model_today.is_empty() { estimated_usd = None; }
    MenuBarUsage {
        today: summary.today.clone(), estimated_usd, unpriced_models,
        pricing_stale: pricing.is_none_or(|pricing| pricing.stale),
        incomplete: summary.unreadable_databases > 0 || summary.skipped_large_records > 0,
    }
}

fn normalized(model: &str) -> String {
    // The native model ID uses -n for the same API model, e.g. gemini-3.8-flash-n.
    let model = model.to_ascii_lowercase();
    model.strip_suffix("-n").unwrap_or(&model).chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn estimate(models: &[LocalTokenModel], prices: &[ApiPricing]) -> (Option<f64>, usize) {
    let mut usd = 0.0;
    let mut priced = 0;
    let mut unpriced = 0;
    for model in models {
        if model.input_tokens == 0 && model.output_tokens == 0 && model.cached_tokens == 0 { continue; }
        let name = normalized(&model.model);
        let mut matching = prices.iter().filter(|price| normalized(&price.model) == name);
        let price = matching.next();
        // Do not guess prices from a substring or silently pick an ambiguous rate.
        if let Some(price) = price.filter(|price| matching.next().is_none() && [price.input, price.output, price.cached].iter().all(|rate| rate.is_finite() && *rate >= 0.0)) {
            usd += (model.input_tokens as f64 * price.input + model.output_tokens as f64 * price.output + model.cached_tokens as f64 * price.cached) / 1_000_000.0;
            priced += 1;
        } else { unpriced += 1; }
    }
    (if priced > 0 || unpriced == 0 { Some(usd) } else { None }, unpriced)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(name: &str) -> LocalTokenModel { LocalTokenModel { model: name.into(), input_tokens: 1_000_000, output_tokens: 100_000, cached_tokens: 2_000_000, total_tokens: 3_100_000, request_count: 3 } }
    fn price() -> ApiPricing { ApiPricing { model: "gemini-3.8-flash".into(), input: 0.75, output: 3.75, cached: 0.075 } }
    #[test]
    fn native_alias_and_three_token_components_use_the_same_rate() {
        let (usd, missing) = estimate(&[model("gemini-3.8-flash-n")], &[price()]);
        assert!((usd.unwrap() - 1.275).abs() < 1e-9); assert_eq!(missing, 0);
    }
    #[test]
    fn unknown_and_ambiguous_models_are_not_free() {
        assert_eq!(estimate(&[model("gemini-3.8-flash-image")], &[price()]), (None, 1));
        assert_eq!(estimate(&[model("gemini-3.8-flash")], &[price(), price()]), (None, 1));
        assert_eq!(estimate(&[model("unknown")], &[]), (None, 1));
        assert_eq!(estimate(&[], &[]), (Some(0.0), 0));
    }
    #[test]
    fn partial_estimate_retains_the_unpriced_count() {
        let (usd, missing) = estimate(&[model("gemini-3.8-flash"), model("unknown")], &[price()]);
        assert!((usd.unwrap() - 1.275).abs() < 1e-9); assert_eq!(missing, 1);
    }
}
