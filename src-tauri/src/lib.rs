mod models;
mod transport;
mod scanner;
mod recommendations;
mod journal;
mod actions;
mod recovery;
mod online;
mod upstream;
mod builder;
mod launcher_variant;
mod wallpapers;

use actions::Engine;
use models::*;
use std::{path::PathBuf,sync::{Arc,Mutex,atomic::AtomicBool}};
use std::path::Path;
use tauri::{Emitter,Manager,State};

#[derive(Clone)]
struct AppState { engine:Engine, operation:Arc<Mutex<()>> }

fn portable_root()->Result<PathBuf,String>{
    #[cfg(debug_assertions)]
    { return Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).ok_or("Cannot locate development root".into()); }
    #[cfg(not(debug_assertions))]
    { std::env::current_exe().map_err(|e|e.to_string())?.parent().map(Path::to_path_buf).ok_or("Cannot locate portable app folder".into()) }
}
fn state(root:PathBuf)->Result<AppState,String>{Ok(AppState{engine:Engine{adb:transport::Adb::new(root.clone())?,journal:journal::Journal::new(&root)?,root,cancelled:Arc::new(AtomicBool::new(false))},operation:Arc::new(Mutex::new(()))})}
fn device_recommendations(root:&Path,snapshot:&DeviceSnapshot)->Vec<Recommendation>{
    let mut recs=recommendations::recommend(snapshot);
    if let Ok((_,ready))=builder::original_path(root){for rec in &mut recs{if rec.action.kind=="install_launcher"&&rec.action.target==actions::ORIGINAL{rec.supported=snapshot.identity.api>=ready.minimum_sdk&&recommendations::capability(snapshot,"Android TV platform")&&recommendations::capability(snapshot,"Stable TV identity");rec.evidence=format!("Official source built and signature-checked on this PC. Version {}. Requires Android API {}+.",ready.version_name,ready.minimum_sdk);rec.already_applied=snapshot.packages.iter().any(|p|p.id==actions::ORIGINAL&&p.installed&&p.version_code>=ready.version_code);}}}
    recs
}
async fn blocking<T:Send+'static>(f:impl FnOnce()->Result<T,String>+Send+'static)->Result<T,String>{tauri::async_runtime::spawn_blocking(f).await.map_err(|e|format!("The operation stopped unexpectedly: {e}"))?}
async fn exclusive<T:Send+'static>(s:AppState,f:impl FnOnce(Engine)->Result<T,String>+Send+'static)->Result<T,String>{blocking(move||{let _guard=s.operation.try_lock().map_err(|_|"Another TV operation is running. Wait for it to finish or stop it between changes.")?;f(s.engine)}).await}

#[tauri::command]
fn runtime_info(s:State<'_,AppState>)->RuntimeInfo{let root=&s.engine.root;RuntimeInfo{version:env!("CARGO_PKG_VERSION").into(),portable_dir:root.to_string_lossy().into_owned(),adb_available:true,launcher_available:root.join("launchers").join(launcher_variant::SWIFTOS_APK_NAME).is_file()||(!cfg!(feature="swiftos-v3-candidate")&&root.join("../cleanyourtv-trial/next-liquid-update/CleanYourTV-MiTV-Liquid-21.apk").is_file()),webview_bundled:root.join("runtime/WebView2/msedgewebview2.exe").is_file(),swiftos_version_code:launcher_variant::SWIFTOS_VERSION_CODE,swiftos_version_name:launcher_variant::SWIFTOS_VERSION_NAME.into(),swiftos_variant:launcher_variant::SWIFTOS_VARIANT.into()}}
#[tauri::command]
async fn list_devices(s:State<'_,AppState>)->Result<Vec<DeviceRef>,String>{let adb=s.engine.adb.clone();blocking(move||adb.devices()).await}
#[tauri::command]
async fn discover_devices(s:State<'_,AppState>)->Result<Vec<DeviceRef>,String>{let adb=s.engine.adb.clone();blocking(move||adb.discover()).await}
#[tauri::command]
async fn connect_device(s:State<'_,AppState>,address:String)->Result<String,String>{let adb=s.engine.adb.clone();blocking(move||adb.connect(&address)).await}
#[tauri::command]
async fn pair_device(s:State<'_,AppState>,address:String,code:String)->Result<String,String>{let adb=s.engine.adb.clone();blocking(move||adb.pair(&address,&code)).await}
#[tauri::command]
async fn wallpaper_status(s:State<'_,AppState>,serial:String)->Result<wallpapers::Status,String>{exclusive(s.inner().clone(),move|e|wallpapers::status(&e,&serial)).await}
#[tauri::command]
async fn upload_wallpapers(app:tauri::AppHandle,s:State<'_,AppState>,serial:String,device_key:String,images:Vec<wallpapers::Upload>)->Result<wallpapers::Uploaded,String>{exclusive(s.inner().clone(),move|e|wallpapers::upload(&e,&serial,&device_key,images,|p|{let _=app.emit("operation-progress",p);})).await}
#[tauri::command]
async fn scan_device(s:State<'_,AppState>,serial:String)->Result<ScanResult,String>{exclusive(s.inner().clone(),move|e|{let snapshot=scanner::scan(&e.adb,&serial)?;e.reconcile(&snapshot)?;e.journal.save_snapshot(&snapshot)?;if recommendations::capability(&snapshot,"Stable TV identity")&&recommendations::capability(&snapshot,"Android TV platform"){e.journal.ensure_baseline(&snapshot)?;}let baseline=e.baseline_status(&snapshot)?;let recommendations=device_recommendations(&e.root,&snapshot);Ok(ScanResult{snapshot,recommendations,baseline})}).await}
#[tauri::command]
async fn preview_plan(s:State<'_,AppState>,serial:String,actions:Vec<ActionRequest>)->Result<PlanPreview,String>{exclusive(s.inner().clone(),move|e|e.preview(&serial,actions)).await}
#[tauri::command]
async fn online_compatibility(s:State<'_,AppState>,serial:String,official_url:Option<String>,include_manual:Option<bool>)->Result<online::OnlineCompatibility,String>{let adb=s.engine.adb.clone();blocking(move||{let snapshot=scanner::scan(&adb,&serial)?;online::check(&snapshot,official_url.as_deref(),include_manual.unwrap_or(false))}).await}
#[tauri::command]
async fn community_status(s:State<'_,AppState>,model:String,refresh:bool)->Result<upstream::CommunityStatus,String>{let root=s.engine.root.clone();blocking(move||upstream::status(&root,&model,refresh)).await}
#[tauri::command]
fn original_launcher_status(s:State<'_,AppState>)->builder::OriginalStatus{builder::status(&s.engine.root)}
#[tauri::command]
fn open_sdk_terms()->Result<(),String>{let mut command=std::process::Command::new("rundll32.exe");command.args(["url.dll,FileProtocolHandler","https://developer.android.com/studio#command-line-tools-only"]);#[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}command.spawn().map(|_|()).map_err(|e|format!("Open the Android SDK terms at developer.android.com/studio: {e}"))}
#[tauri::command]
async fn prepare_original_launcher(app:tauri::AppHandle,s:State<'_,AppState>,serial:String,accept_sdk_terms:bool)->Result<builder::OriginalStatus,String>{exclusive(s.inner().clone(),move|e|{let scan=scanner::scan(&e.adb,&serial)?;if scan.identity.api<26||!recommendations::capability(&scan,"Android TV platform"){return Err("This launcher needs a verified Android TV platform, API 26 or newer.".into());}builder::prepare(&e,accept_sdk_terms,|p|{let _=app.emit("operation-progress",p);})}).await}
#[tauri::command]
async fn apply_plan(app:tauri::AppHandle,s:State<'_,AppState>,serial:String,plan:PlanPreview)->Result<ApplyResult,String>{exclusive(s.inner().clone(),move|e|e.apply(&serial,plan,|p|{let _=app.emit("operation-progress",p);})).await}
#[tauri::command]
async fn confirm_home(s:State<'_,AppState>,serial:String,token:String)->Result<ChangeRecord,String>{exclusive(s.inner().clone(),move|e|e.confirm_home(&serial,&token)).await}
#[tauri::command]
fn cancel_operations(s:State<'_,AppState>){s.engine.cancelled.store(true,std::sync::atomic::Ordering::SeqCst);}
#[tauri::command]
async fn history(s:State<'_,AppState>,device_key:Option<String>)->Result<Vec<ChangeRecord>,String>{let j=s.engine.journal.clone();blocking(move||j.list(device_key.as_deref())).await}
#[tauri::command]
async fn restore_change(s:State<'_,AppState>,serial:String,change_id:String)->Result<ChangeRecord,String>{exclusive(s.inner().clone(),move|e|e.restore(&serial,&change_id)).await}
#[tauri::command]
async fn preview_restore_all(s:State<'_,AppState>,serial:String)->Result<RestoreAllPreview,String>{exclusive(s.inner().clone(),move|e|e.preview_restore_all(&serial)).await}
#[tauri::command]
async fn restore_all(app:tauri::AppHandle,s:State<'_,AppState>,serial:String,plan:RestoreAllPreview)->Result<Vec<ChangeRecord>,String>{exclusive(s.inner().clone(),move|e|e.restore_all(&serial,plan,|p|{let _=app.emit("operation-progress",p);})).await}
#[tauri::command]
async fn restore_session(s:State<'_,AppState>,serial:String,session_id:String)->Result<Vec<ChangeRecord>,String>{exclusive(s.inner().clone(),move|e|{let snap=scanner::scan(&e.adb,&serial)?;e.reconcile(&snap)?;let records=e.journal.list(Some(&snap.identity.device_key))?;let mut restored=vec![];for r in records.into_iter().filter(|r|r.session_id==session_id&&r.restore_status=="available"&&!matches!(r.status.as_str(),"prepared"|"failed")){restored.push(e.restore(&serial,&r.id)?);}Ok(restored)}).await}
#[tauri::command]
async fn export_report(s:State<'_,AppState>,device_key:String)->Result<String,String>{let e=s.engine.clone();blocking(move||{
    if device_key.len()!=64||!device_key.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Invalid TV identity".into());}
    let snapshot=e.journal.snapshot(&device_key)?;let changes=e.journal.list(Some(&device_key))?;
    let data=serde_json::json!({"schema_version":1,"application":"TV Optimizer","version":env!("CARGO_PKG_VERSION"),"exported_at":transport::now_ms(),"device_key":device_key,"snapshot":snapshot,"starting_snapshot":e.journal.baseline(&device_key)?,"changes":changes,"limitations":["Private app data and sign-ins are not included.","A backup applies only to its recorded TV, user and firmware.","Read-only observations are not a full firmware backup.","Restore all uses the original local starting snapshot; importing a report does not overwrite it."]});
    let dir=e.root.join("data/exports");std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;let path=dir.join(format!("tv-report-{}.json",transport::now_ms()));std::fs::write(&path,serde_json::to_vec_pretty(&data).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;Ok(path.to_string_lossy().into_owned())
}).await}
#[tauri::command]
async fn import_history(s:State<'_,AppState>,serial:String,path:String)->Result<usize,String>{exclusive(s.inner().clone(),move|e|{
    let p=std::path::Path::new(&path);if !p.is_file()||p.extension().and_then(|x|x.to_str())!=Some("json"){return Err("Choose a TV Optimizer JSON export. Shell scripts are not imported or executed.".into());}
    let metadata=std::fs::metadata(p).map_err(|e|e.to_string())?;if metadata.len()>32*1024*1024{return Err("The recovery export is too large.".into());}
    import_records(e,&serial,&std::fs::read(p).map_err(|e|e.to_string())?)
}).await}
#[tauri::command]
async fn import_history_data(s:State<'_,AppState>,serial:String,json:String)->Result<usize,String>{exclusive(s.inner().clone(),move|e|import_records(e,&serial,json.as_bytes())).await}
fn import_records(e:Engine,serial:&str,data:&[u8])->Result<usize,String>{
    if data.len()>32*1024*1024{return Err("The recovery export is too large.".into());}
    let v:serde_json::Value=serde_json::from_slice(data).map_err(|_|"This is not a valid recovery JSON file")?;
    if v["schema_version"]!=1||v["application"]!="TV Optimizer"{return Err("Unsupported recovery export format. Original PowerShell backups are preserved separately and never executed by import.".into());}
    let snap=scanner::scan(&e.adb,&serial)?;
    if v["device_key"].as_str()!=Some(&snap.identity.device_key){return Err("This export belongs to another TV or user.".into());}
    let records:Vec<ChangeRecord>=serde_json::from_value(v["changes"].clone()).map_err(|_|"Invalid change records")?;
    if records.len()>20_000{return Err("Too many records in this export".into());}
    for r in &records{
        if uuid::Uuid::parse_str(&r.id).is_err()||uuid::Uuid::parse_str(&r.session_id).is_err()||r.device_key!=snap.identity.device_key||r.user_id!=snap.identity.user_id||r.model!=snap.identity.model||r.fingerprint!=snap.identity.fingerprint{return Err("A record does not match this TV and firmware.".into());}
        if !["disable","enable","uninstall","force_stop","animations","dns","install_launcher","set_home","patchwall_startup","trim_cache","wallpaper_auto"].contains(&r.kind.as_str()){return Err("The export includes an unsupported operation.".into());}
        let _:serde_json::Value=serde_json::from_str(&r.before_json).map_err(|_|"Invalid before-state")?;let _:serde_json::Value=serde_json::from_str(&r.after_json).map_err(|_|"Invalid after-state")?;
        recovery::validate_record(r)?;
        if e.journal.get(&r.id).is_ok(){return Err("This export contains a restore point already present. Existing recovery records are never overwritten.".into());}
    }
    for r in &records{e.journal.save_imported(r)?;}Ok(records.len())
}

pub fn run(){
    let root=portable_root().expect("Cannot locate the portable app");
    // Use the copy beside this executable, so no machine-wide runtime installation is required.
    let runtime=root.join("runtime/WebView2");
    if runtime.join("msedgewebview2.exe").is_file(){std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",&runtime);}
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER",root.join("data/webview"));
    let s=match state(root.clone()){Ok(s)=>s,Err(e)=>{let _=std::fs::write(root.join("startup-error.txt"),&e);panic!("{e}");}};
    // Automated QA uses this read-only native route. No optimization operation is exposed here.
    let args:Vec<String>=std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("--community-check"){
        let result=upstream::status(&root,args.get(2).map(String::as_str).unwrap_or(""),true);let payload=match result{Ok(r)=>serde_json::to_value(r).unwrap(),Err(e)=>serde_json::json!({"error":e})};let _=std::fs::write(root.join("data/native-community.json"),serde_json::to_vec_pretty(&payload).unwrap());return;
    }
    if matches!(args.get(1).map(String::as_str),Some("--scan-only"|"--manufacturer-check")){
        let result=args.get(2).ok_or("Specify the authorized TV serial".into()).and_then(|serial|scanner::scan(&s.engine.adb,serial));
        let manufacturer=args.get(1).map(String::as_str)==Some("--manufacturer-check");
        let path=root.join(if manufacturer{"data/native-manufacturer.json"}else{"data/native-scan.json"});
        let payload=match result{Ok(s)=>if manufacturer{match online::check(&s,None,true){Ok(r)=>serde_json::to_value(r).unwrap(),Err(e)=>serde_json::json!({"error":e})}}else{serde_json::json!({"snapshot":s,"recommendations":device_recommendations(&root,&s)})},Err(e)=>serde_json::json!({"error":e})};let _=std::fs::write(path,serde_json::to_vec_pretty(&payload).unwrap());return;
    }
    tauri::Builder::default().manage(s).invoke_handler(tauri::generate_handler![runtime_info,list_devices,discover_devices,connect_device,pair_device,scan_device,wallpaper_status,upload_wallpapers,online_compatibility,community_status,original_launcher_status,open_sdk_terms,prepare_original_launcher,preview_plan,apply_plan,confirm_home,history,restore_change,restore_session,preview_restore_all,restore_all,cancel_operations,export_report,import_history,import_history_data]).setup(|app|{if let Some(window)=app.get_webview_window("main"){let _=window.set_title("TV Optimizer");}Ok(())}).run(tauri::generate_context!()).expect("TV Optimizer could not open its window");
}
