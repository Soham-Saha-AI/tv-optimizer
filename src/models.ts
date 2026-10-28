export interface DeviceRef { serial:string; state:string; model:string; product:string }
export interface DeviceIdentity { serial:string; device_key:string; model:string; manufacturer:string; android_version:string; api:number; abi:string; fingerprint:string; user_id:number }
export interface PackageInfo { id:string; label:string; installed:boolean; enabled:number|null; system:boolean; version_name:string; version_code:number; apk_paths:string[]; is_launcher:boolean; protected:boolean; reason:string; purpose?:string }
export interface Inspection { name:string; status:string; detail:string }
export interface DeviceSnapshot { identity:DeviceIdentity; scanned_at:number; profile:string|null; packages:PackageInfo[]; home_component:string|null; settings:Record<string,string|null>; memory_total_kb:number|null; memory_available_kb:number|null; storage_free_kb:number|null; processes:string; inspections:Inspection[] }
export type ActionKind='disable'|'enable'|'uninstall'|'force_stop'|'animations'|'dns'|'install_launcher'|'set_home'|'patchwall_startup'|'trim_cache'|'wallpaper_auto';
export interface ActionRequest { kind:ActionKind; target:string; value:string }
export interface Recommendation { id:string; area:string; title:string; description:string; evidence:string; impact:string; restore:string; risk:'low'|'medium'|'high'; supported:boolean; already_applied:boolean; action:ActionRequest }
export interface PlanPreview { device_key:string; fingerprint:string; actions:ActionRequest[]; warnings:string[]; created_at:number; preconditions:Record<string,string> }
export interface ChangeRecord { id:string; session_id:string; device_key:string; serial:string; model:string; fingerprint:string; user_id:number; kind:string; target:string; label:string; before_json:string; after_json:string; status:string; restore_status:string; error:string|null; created_at:number; updated_at:number }
export interface OperationProgress { phase:string; message:string; completed:number; total:number; change_id:string|null }
export interface ApplyResult { session_id:string; changes:ChangeRecord[]; requires_home_confirmation:boolean; home_token:string|null }
export interface RuntimeInfo { version:string; portable_dir:string; adb_available:boolean; launcher_available:boolean; webview_bundled:boolean; swiftos_version_code:number; swiftos_version_name:string; swiftos_variant:string }
export interface BaselineStatus { saved_at:number; model:string; pre_existing:string[]; restorable_changes:number; non_reversible_changes:number; review_required:number }
export interface RestoreAllPreview { device_key:string; fingerprint:string; baseline_at:number; change_ids:string[]; labels:string[]; skipped:string[]; created_at:number; preconditions:Record<string,string> }
export interface ScanResult { snapshot:DeviceSnapshot; recommendations:Recommendation[]; baseline?:BaselineStatus|null }
export interface BasketItem { action:ActionRequest; title:string; impact:string; restore:string; risk:'low'|'medium'|'high' }
export type PageId='overview'|'apps'|'launchers'|'ads'|'performance'|'wallpapers'|'restore'|'setup';
export interface OnlineSource {title:string;url:string;status:'matched'|'reference_only'|'unavailable';detail:string}
export interface OnlineCompatibility {model:string;checked_at:number;sources:OnlineSource[];findings:string[];remote_checks:string[];summary:string}
