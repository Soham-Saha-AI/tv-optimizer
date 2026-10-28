# Implementation contract

Rust serde and TypeScript use snake_case properties. Commands use Tauri invoke with argument objects in camelCase where Tauri expects it. Never execute arbitrary command strings received from the frontend.

## Models (canonical src-tauri/src/models.rs)

DeviceRef { serial:string, state:string, model:string, product:string }
DeviceIdentity { serial, device_key, model, manufacturer, android_version, api:number, abi, fingerprint, user_id:number }
PackageInfo { id, label, installed:boolean, enabled:number|null, system:boolean, version_name, version_code:number, apk_paths:string[], is_launcher:boolean, protected:boolean, reason:string }
Inspection { name, status:'detected'|'already_optimized'|'needs_attention'|'could_not_check', detail }
DeviceSnapshot { identity, scanned_at:number milliseconds, profile:string|null, packages:PackageInfo[], home_component:string|null, settings:map<string,string|null>, memory_total_kb:number|null, memory_available_kb:number|null, storage_free_kb:number|null, processes:string, inspections:Inspection[] }
ActionRequest { kind:string, target:string, value:string } // typed whitelist: disable, enable, uninstall, force_stop, animations, dns, install_launcher, set_home, patchwall_startup, trim_cache
Recommendation { id, area, title, description, evidence, impact, restore, risk:'low'|'medium'|'high', supported:boolean, already_applied:boolean, action:ActionRequest }
PlanPreview { device_key, fingerprint, actions:ActionRequest[], warnings:string[], created_at:number }
ChangeRecord { id, session_id, device_key, serial, model, fingerprint, user_id:number, kind, target, label, before_json:string, after_json:string, status, restore_status, error:string|null, created_at:number, updated_at:number }
OperationProgress { phase, message, completed:number,total:number, change_id:string|null }
ApplyResult { session_id, changes:ChangeRecord[], requires_home_confirmation:boolean, home_token:string|null }
RuntimeInfo { version, portable_dir, adb_available:boolean, launcher_available:boolean, webview_bundled:boolean }

## Backend module ownership

- scanner agent: transport.rs + scanner.rs + recommendations.rs and their unit tests. Root owns models.rs; request additions through message. Scanner exports scan(&Adb, serial:&str)->Result<DeviceSnapshot,String>; recommendations::recommend(&DeviceSnapshot)->Vec<Recommendation>.
- transport exports Adb::new(root:PathBuf)->Result<Self,String>; devices()->Result<Vec<DeviceRef>,String>; connect(address:&str), pair(address:&str,code:&str)->Result<String,String>; discover()->Result<Vec<DeviceRef>,String>; shell(serial:&str,command:&str)->Result<String,String>; run(args:&[&str])->Result<String,String>; push(serial:&str,local:&Path,remote:&str); pull(...). All operations explicit -s except host discovery/connect. Adb is Clone+Send+Sync. Export quote(&str)->String for shell single quote escaping, validate_package(&str)->bool, now_ms()->i64.
- root: models.rs, actions.rs, journal.rs, lib.rs/main.rs, Cargo.toml, Tauri config; frontend agent owns src frontend and package.json/Vite/TS configs.

## IPC commands (all Result<...,String>)

runtime_info()->RuntimeInfo
list_devices()->DeviceRef[]
discover_devices()->DeviceRef[]
connect_device(address)->string
pair_device(address,code)->string
scan_device(serial)->{snapshot:DeviceSnapshot,recommendations:Recommendation[],baseline:BaselineStatus|null}
preview_plan(serial,actions:ActionRequest[])->PlanPreview // fresh scan and validate only
apply_plan(serial,plan:PlanPreview)->ApplyResult // fresh identity/state guard, durable backups, sequential
confirm_home(serial,token)->ChangeRecord
history(deviceKey?:string)->ChangeRecord[]
restore_change(serial,changeId)->ChangeRecord
restore_session(serial,sessionId)->ChangeRecord[]
preview_restore_all(serial)->RestoreAllPreview // read-only chain simulation and initial baseline check
restore_all(serial,plan:RestoreAllPreview)->ChangeRecord[] // exact fresh preview, reverse local history only
cancel_operations()->void
export_report(deviceKey:string)->string // export to data/exports, return path
import_history(serial,path:string)->number // strict schema+identity, no scripts

Progress event `operation-progress`: OperationProgress. All actions initially unselected. No action controls enabled in browser without Tauri; preview demo is explicit and does not imply device connection. Preview UI can have built-in demo mode for QA using clearly labeled sample data and no TV writes.

## Safety + supported initial actions

Profile MiTV-AXSO1 API28 fingerprint recorded PI.6157; package writes require profile match and allowed safe targets. Protect fallback settings systemui network input BT media/DRM Play/store/core services. Safe disables analytics/feedback/bugreportsender and explicit optional YouTube Music/Play Movies. Launchers separately Google com.google.android.tvlauncher, Xiaomi com.mitv.tvhome, custom com.gathin.tvhome. Do not offer unknown packages as safe recommendations. Generic read-only scans + standard animation settings are allowed outside model profile; package changes use validated profile. Exact enabled states 0/1/2/3/4, installed-for-user flags and absent settings preserved.

Launcher switching: verify installed target, choose recorded previous valid home, start detached 150sec recovery before takeover, user confirmation gate before disabling old homes; any package launcher disable requires verified confirmed custom Home in current session or record. User can confirm already installed custom Home through set_home flow. Guard FallbackHome.

Scope includes history+restore+group undo, DNS transaction rollback, system-app user uninstall -k, APK backup for thirdparty uninstall when eligible, cache cleanup nonundoable and force-stop temporary. Root can mark unsupported if platform capability truly unavailable, no fake success. DNS standard=dns.adguard-dns.com stronger=x-hagezi-pro.freedns.controld.com. Scan settings use keys global/window_animation_scale, global/transition_animation_scale, global/animator_duration_scale, global/private_dns_mode, global/private_dns_specifier, global/set_patchwall_default, secure/enabled_accessibility_services, secure/accessibility_enabled.
