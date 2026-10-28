use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceRef { pub serial:String, pub state:String, pub model:String, pub product:String }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceIdentity { pub serial:String, pub device_key:String, pub model:String, pub manufacturer:String, pub android_version:String, pub api:u32, pub abi:String, pub fingerprint:String, pub user_id:u32 }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageInfo { pub id:String, pub label:String, pub installed:bool, pub enabled:Option<u32>, pub system:bool, pub version_name:String, pub version_code:u64, pub apk_paths:Vec<String>, pub is_launcher:bool, pub protected:bool, pub reason:String, #[serde(default)] pub purpose:String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection { pub name:String, pub status:String, pub detail:String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSnapshot { pub identity:DeviceIdentity, pub scanned_at:i64, pub profile:Option<String>, pub packages:Vec<PackageInfo>, pub home_component:Option<String>, pub settings:BTreeMap<String,Option<String>>, pub memory_total_kb:Option<u64>, pub memory_available_kb:Option<u64>, pub storage_free_kb:Option<u64>, pub processes:String, pub inspections:Vec<Inspection> }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRequest { pub kind:String, pub target:String, pub value:String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation { pub id:String, pub area:String, pub title:String, pub description:String, pub evidence:String, pub impact:String, pub restore:String, pub risk:String, pub supported:bool, pub already_applied:bool, pub action:ActionRequest }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPreview { pub device_key:String, pub fingerprint:String, pub actions:Vec<ActionRequest>, pub warnings:Vec<String>, pub created_at:i64, #[serde(default)] pub preconditions:BTreeMap<String,String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord { pub id:String, pub session_id:String, pub device_key:String, pub serial:String, pub model:String, pub fingerprint:String, pub user_id:u32, pub kind:String, pub target:String, pub label:String, pub before_json:String, pub after_json:String, pub status:String, pub restore_status:String, pub error:Option<String>, pub created_at:i64, pub updated_at:i64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationProgress { pub phase:String, pub message:String, pub completed:usize, pub total:usize, pub change_id:Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult { pub session_id:String, pub changes:Vec<ChangeRecord>, pub requires_home_confirmation:bool, pub home_token:Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo { pub version:String, pub portable_dir:String, pub adb_available:bool, pub launcher_available:bool, pub webview_bundled:bool, pub swiftos_version_code:u64, pub swiftos_version_name:String, pub swiftos_variant:String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult { pub snapshot:DeviceSnapshot, pub recommendations:Vec<Recommendation>, pub baseline:Option<BaselineStatus> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineStatus { pub saved_at:i64, pub model:String, pub pre_existing:Vec<String>, pub restorable_changes:usize, pub non_reversible_changes:usize, pub review_required:usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreAllPreview { pub device_key:String, pub fingerprint:String, pub baseline_at:i64, pub change_ids:Vec<String>, pub labels:Vec<String>, pub skipped:Vec<String>, pub created_at:i64, pub preconditions:BTreeMap<String,String> }
