#!/usr/bin/env node
// One-time, explicitly approved macOS acceptance test. Never launches an App,
// changes a port, reads credentials/chat/input values, or clicks a control.
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const failure = (code) => Object.assign(new Error(code), { safeCode: code });
const must = (condition, code) => { if (!condition) throw failure(code); };
const idOK = (id) => typeof id === 'string' && /^[A-Za-z0-9-]{1,128}$/.test(id);
const VERSION = '2.19.1';
let candidateRuntime=null, candidateDictionary=null;
const LIMIT = 256 * 1024;

export function parseListeners(text, port) {
  let pid = null;
  const rows = [];
  for (const line of text.split('\n')) {
    if (/^p[0-9]+$/.test(line)) pid = Number(line.slice(1));
    if (line.startsWith('n')) {
      must(pid > 0, 'listener_owner_missing');
      const name = line.slice(1);
      must(name === `127.0.0.1:${port}` || name === `[::1]:${port}`, 'listener_not_loopback');
      rows.push({ pid, address: name });
    }
  }
  must(rows.length > 0 && rows.length <= 8, 'listener_not_verified');
  must(rows.some(row => row.address === `127.0.0.1:${port}`), 'ipv4_listener_missing');
  return rows;
}

export function parsePortFile(text) {
  must(text.length <= 256, 'port_file_too_large');
  const lines = text.trimEnd().split(/\r?\n/);
  must(lines.length === 2 && /^[1-9][0-9]{0,4}$/.test(lines[0]), 'port_file_invalid');
  const port = Number(lines[0]);
  must(port <= 65535, 'port_file_invalid');
  const prefix = '/devtools/browser/';
  must(lines[1].startsWith(prefix) && idOK(lines[1].slice(prefix.length)), 'endpoint_invalid');
  return { port, endpoint: `ws://127.0.0.1:${port}${lines[1]}` };
}

// Returned data contains only shape/known-label booleans, never arbitrary text.
export function labelExpression(origin, apply) {
  must(candidateRuntime&&candidateDictionary,'candidate_files_missing');
  const config=JSON.stringify({appVersion:VERSION,locale:'zh-CN',dictionary:candidateDictionary});
  return `(async () => {
    if(location.origin!==${JSON.stringify(origin)}) return {ok:false,code:'origin_changed'};
    if(window.__ANTIGRAVITY_TOOLS_LOCALIZATION__) return {ok:false,code:'existing_controller'};
    const config=${config};
    const fresh=()=>(${candidateRuntime})({document:window.document,MutationObserver:window.MutationObserver,setTimeout:window.setTimeout.bind(window),clearTimeout:window.clearTimeout.bind(window)},config);
    const before=fresh().probe();
    const nav=before.scopes?.find(s=>s.id==='settings-navigation');
    if(before.status!=='supported'||!nav?.present||nav.labelCount<2) return {ok:false,code:'navigation_not_verified'};
    if(!${JSON.stringify(apply)})return {ok:true,shape_verified:true,english:true,label_count:before.labelCount,navigation_labels:nav.labelCount};
    const controller=(${candidateRuntime})(window,config);
    let applied=null, observed=null, restored=null;
    try {
      applied=controller.apply();
      if(applied.status!=='applied'||!applied.active||applied.translated!==before.labelCount) return {ok:false,code:'apply_not_verified'};
      await new Promise(resolve=>setTimeout(resolve,2000));
      observed=controller.probe();
    } finally {restored=controller.dispose();}
    const after=fresh().probe();
    return {ok:observed?.status==='supported'&&restored?.active===false&&after.status==='supported'&&after.labelCount===before.labelCount,changed:applied?.translated>0,restored:restored?.active===false&&after.status==='supported'&&after.labelCount===before.labelCount,label_count:before.labelCount,navigation_labels:nav.labelCount};
  })()`;
}

function command(file, args) {
  try { return execFileSync(file, args, { encoding: 'utf8', timeout: 3000, maxBuffer: LIMIT, stdio: ['ignore','pipe','pipe'] }).trim(); }
  catch { throw failure('required_metadata_command_failed'); }
}
function sameExecutable(pid, expected) {
  const actual = command('/bin/ps', ['-p', String(pid), '-o', 'comm=']);
  try { return fs.realpathSync(actual) === fs.realpathSync(expected); } catch { return false; }
}
function listeners(port) {
  return parseListeners(command('/usr/sbin/lsof', ['-nP','-iTCP:'+port,'-sTCP:LISTEN','-Fpn']), port);
}

async function connect(endpoint) {
  must(typeof WebSocket === 'function' && Number(process.versions.node.split('.')[0]) >= 22, 'node_22_required');
  must(!process.env.NODE_USE_ENV_PROXY && !process.env.NODE_OPTIONS, 'node_preload_or_proxy_not_allowed');
  const ws = new WebSocket(endpoint);
  const pending = new Map(); let counter = 0;
  const ready = new Promise((resolve, reject) => {
    const timer = setTimeout(() => { ws.close(); reject(failure('cdp_connect_timeout')); }, 3000);
    ws.addEventListener('open', () => { clearTimeout(timer); resolve(); }, { once: true });
    ws.addEventListener('error', () => { clearTimeout(timer); reject(failure('cdp_connection_failed')); }, { once: true });
  });
  ws.addEventListener('message', event => {
    if(typeof event.data!=='string'||event.data.length>LIMIT){ws.close();return;}
    let message; try { message=JSON.parse(event.data); } catch { ws.close(); return; }
    const waiter=pending.get(message.id); if(!waiter)return;
    pending.delete(message.id); clearTimeout(waiter.timer);
    if(message.error) waiter.reject(failure('cdp_protocol_error')); else waiter.resolve(message.result);
  });
  ws.addEventListener('close',()=>{
    for(const waiter of pending.values()){clearTimeout(waiter.timer);waiter.reject(failure('cdp_closed'));}
    pending.clear();
  });
  await ready;
  return {
    call(method,params={},sessionId){
      must(['SystemInfo.getProcessInfo','Target.getTargets','Target.getTargetInfo','Target.attachToTarget','Target.detachFromTarget','Runtime.evaluate'].includes(method),'unapproved_method');
      const id=++counter;
      return new Promise((resolve,reject)=>{
        const timer=setTimeout(()=>{pending.delete(id);reject(failure('cdp_timeout'));},7000);
        pending.set(id,{resolve,reject,timer});
        ws.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));
      });
    }, close(){ws.close();}
  };
}

export async function main(argv) {
  must(process.platform==='darwin','macos_only');
  const args={};
  for(let i=0;i<argv.length;i+=2){must(argv[i]?.startsWith('--')&&argv[i+1],'arguments_invalid');args[argv[i]]=argv[i+1];}
  must(Object.keys(args).every(k=>['--app','--pid','--port-file','--mode','--runtime','--dictionary'].includes(k)),'arguments_invalid');
  must(args['--mode']==='probe'||args['--mode']==='approved-flip-once','explicit_mode_required');
  const runtime=fs.readFileSync(args['--runtime']||'');
  const dictionary=fs.readFileSync(args['--dictionary']||'');
  must(createHash('sha256').update(runtime).digest('hex')==='cb291288529848f1b104339917870028bbb230b500c978bd3c61bd10492cbe86','runtime_hash_mismatch');
  must(createHash('sha256').update(dictionary).digest('hex')==='4e8e7833c206996ec0d2aaaab1b4c535e72ec94cd988abf7abf1d3ce5e496312','dictionary_hash_mismatch');
  const marker='const ENABLE_SOURCE_DERIVED_NAVIGATION = false; // ENABLE_ONLY_AFTER_LIVE_NAVIGATION_QA';
  const text=runtime.toString('utf8');must(text.split(marker).length===2,'candidate_marker_invalid');
  candidateRuntime=text.replace(marker,'const ENABLE_SOURCE_DERIVED_NAVIGATION = true; // APPROVED_TEMPORARY_QA_ONLY');
  candidateDictionary=JSON.parse(dictionary.toString('utf8'));
  const app=fs.realpathSync(args['--app']||'');
  const pid=Number(args['--pid']); must(Number.isInteger(pid)&&pid>0,'app_pid_required');
  const plist=path.join(app,'Contents/Info.plist');
  must(command('/usr/libexec/PlistBuddy',['-c','Print :CFBundleShortVersionString',plist])===VERSION,'unsupported_version');
  const executable=command('/usr/libexec/PlistBuddy',['-c','Print :CFBundleExecutable',plist]);
  must(executable==='Antigravity','wrong_app_identity');
  must(sameExecutable(pid,path.join(app,'Contents/MacOS',executable)),'app_pid_not_verified');
  const portPath=args['--port-file']; must(portPath&&path.basename(portPath)==='DevToolsActivePort','exact_port_file_required');
  const stat=fs.lstatSync(portPath);must(stat.isFile()&&stat.size<=256,'port_file_invalid');
  const {port,endpoint}=parsePortFile(fs.readFileSync(portPath,'utf8'));
  must(listeners(port).every(row=>row.pid===pid),'debug_port_wrong_owner');
  const cdp=await connect(endpoint); let activeSession=null;
  try {
    const verify=async()=>{
      const info=await cdp.call('SystemInfo.getProcessInfo');
      const browsers=(info.processInfo||[]).filter(p=>p.type==='browser');
      must(browsers.length===1&&browsers[0].id===pid,'cdp_pid_not_verified');
    };
    await verify();
    const info=await cdp.call('Target.getTargets',{filter:[{type:'page',exclude:false}]});
    must(Array.isArray(info.targetInfos)&&info.targetInfos.length<=32,'targets_invalid');
    const candidates=[];
    for(const target of info.targetInfos){
      if(target.type!=='page'||!idOK(target.targetId))continue;
      let url;try{url=new URL(target.url);}catch{continue;}
      if(url.protocol!=='https:'||url.hostname!=='127.0.0.1'||url.username||url.password||!url.port||!target.url.startsWith(url.origin+'/'))continue;
      const rows=listeners(Number(url.port));
      const owners=[...new Set(rows.map(row=>row.pid))];
      must(owners.length===1,'app_server_owner_ambiguous');
      const serverPid=owners[0];
      must(sameExecutable(serverPid,path.join(app,'Contents/Resources/bin/language_server')),'app_server_not_verified');
      must(Number(command('/bin/ps',['-p',String(serverPid),'-o','ppid=']))===pid,'app_server_parent_mismatch');
      candidates.push({id:target.targetId,origin:url.origin});
    }
    must(candidates.length>0&&candidates.length<=8,'no_verified_app_page');
    const matches=[];
    for(const candidate of candidates){
      const attached=await cdp.call('Target.attachToTarget',{targetId:candidate.id,flatten:true});
      must(idOK(attached.sessionId),'session_invalid');activeSession=attached.sessionId;
      const raw=await cdp.call('Runtime.evaluate',{expression:labelExpression(candidate.origin,false),returnByValue:true,awaitPromise:true,timeout:6000},activeSession);
      must(!raw.exceptionDetails,'probe_exception');
      if(raw.result?.value?.ok===true)matches.push(candidate);
      await cdp.call('Target.detachFromTarget',{sessionId:activeSession});activeSession=null;
    }
    must(matches.length===1,'settings_control_not_unique_or_verified');
    if(args['--mode']==='probe')return {ok:true,app_version:VERSION,process_and_loopback_verified:true,shape_verified:true,navigation_scope_verified:true,changed:false};
    const target=matches[0]; await verify();
    const fresh=await cdp.call('Target.getTargetInfo',{targetId:target.id});
    must(fresh.targetInfo?.type==='page'&&new URL(fresh.targetInfo.url).origin===target.origin,'target_changed');
    const attached=await cdp.call('Target.attachToTarget',{targetId:target.id,flatten:true});
    must(idOK(attached.sessionId),'session_invalid');activeSession=attached.sessionId;
    const raw=await cdp.call('Runtime.evaluate',{expression:labelExpression(target.origin,true),returnByValue:true,awaitPromise:true,timeout:6000},activeSession);
    must(!raw.exceptionDetails&&raw.result?.value?.ok===true&&raw.result.value.restored===true,'flip_or_restore_not_verified');
    const check=await cdp.call('Runtime.evaluate',{expression:labelExpression(target.origin,false),returnByValue:true,awaitPromise:true,timeout:6000},activeSession);
    must(!check.exceptionDetails&&check.result?.value?.ok===true,'restoration_postcheck_failed');
    return {ok:true,app_version:VERSION,process_and_loopback_verified:true,shape_verified:true,changed:true,restored:true,english_postcheck:true,label_count:raw.result.value.label_count,navigation_labels:raw.result.value.navigation_labels,duration_ms:2000};
  } finally {
    if(activeSession){try{await cdp.call('Target.detachFromTarget',{sessionId:activeSession});}catch{}}
    cdp.close();
  }
}

if(process.argv[1]&&pathToFileURL(path.resolve(process.argv[1])).href===import.meta.url){
  main(process.argv.slice(2)).then(value=>console.log(JSON.stringify(value))).catch(error=>{
    console.log(JSON.stringify({ok:false,code:error.safeCode||'blocked',changed_or_restored:'unknown_if_flip_was_started'}));process.exitCode=1;
  });
}
