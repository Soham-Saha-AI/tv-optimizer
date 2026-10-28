use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Journal { path:PathBuf }

impl Journal {
    pub fn new(root:&Path)->Result<Self,String> {
        let data=root.join("data");
        std::fs::create_dir_all(&data).map_err(|e| format!("Cannot save recovery history beside the app. Extract it to a writable folder: {e}"))?;
        let journal=Self { path:data.join("history.sqlite3") };
        let db=journal.open()?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS changes(id TEXT PRIMARY KEY, device_key TEXT NOT NULL, session_id TEXT NOT NULL, created_at INTEGER NOT NULL, payload TEXT NOT NULL); CREATE INDEX IF NOT EXISTS by_device ON changes(device_key, created_at); CREATE TABLE IF NOT EXISTS snapshots(device_key TEXT PRIMARY KEY, payload TEXT NOT NULL); CREATE TABLE IF NOT EXISTS baselines(device_key TEXT PRIMARY KEY, payload TEXT NOT NULL); CREATE TABLE IF NOT EXISTS baseline_members(change_id TEXT PRIMARY KEY, device_key TEXT NOT NULL, created_at INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS baseline_by_device ON baseline_members(device_key,created_at);").map_err(|e|e.to_string())?;
        Ok(journal)
    }
    fn open(&self)->Result<Connection,String> {
        let db=Connection::open(&self.path).map_err(|e|e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(10)).map_err(|e|e.to_string())?;
        db.pragma_update(None,"journal_mode","WAL").map_err(|e|e.to_string())?;
        db.pragma_update(None,"synchronous","FULL").map_err(|e|e.to_string())?;
        Ok(db)
    }
    pub fn save(&self,record:&ChangeRecord)->Result<(),String> {
        self.save_record(record,true)
    }
    /// Imported history remains individually restorable, but is never attributed
    /// to changes made by this portable copy after its starting restore point.
    pub fn save_imported(&self,record:&ChangeRecord)->Result<(),String> {
        self.save_record(record,false)
    }
    fn save_record(&self,record:&ChangeRecord,enroll:bool)->Result<(),String> {
        let mut db=self.open()?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        let existed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM changes WHERE id=?1)",[&record.id],|r|r.get(0)).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO changes(id,device_key,session_id,created_at,payload) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",params![record.id,record.device_key,record.session_id,record.created_at,serde_json::to_string(record).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        if enroll&&!existed {
            let baseline:Option<String>=tx.query_row("SELECT payload FROM baselines WHERE device_key=?1",[&record.device_key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            if let Some(payload)=baseline {
                let baseline:DeviceSnapshot=serde_json::from_str(&payload).map_err(|e|e.to_string())?;
                if record.created_at>=baseline.scanned_at {
                    tx.execute("INSERT INTO baseline_members(change_id,device_key,created_at) VALUES(?1,?2,?3)",params![record.id,record.device_key,record.created_at]).map_err(|e|e.to_string())?;
                }
            }
        }
        tx.commit().map_err(|e|e.to_string())
    }
    pub fn get(&self,id:&str)->Result<ChangeRecord,String> {
        let data:String=self.open()?.query_row("SELECT payload FROM changes WHERE id=?1",[id],|r|r.get(0)).map_err(|_|"This restore point was not found.".to_string())?;
        serde_json::from_str(&data).map_err(|e|e.to_string())
    }
    pub fn list(&self,key:Option<&str>)->Result<Vec<ChangeRecord>,String> {
        let db=self.open()?;
        let mut st=db.prepare("SELECT payload FROM changes WHERE (?1 IS NULL OR device_key=?1) ORDER BY created_at DESC, rowid DESC").map_err(|e|e.to_string())?;
        let rows=st.query_map(params![key],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        rows.map(|r|r.map_err(|e|e.to_string()).and_then(|s|serde_json::from_str(&s).map_err(|e|e.to_string()))).collect()
    }
    pub fn save_snapshot(&self,snapshot:&DeviceSnapshot)->Result<(),String> {
        self.open()?.execute("INSERT INTO snapshots(device_key,payload) VALUES(?1,?2) ON CONFLICT(device_key) DO UPDATE SET payload=excluded.payload",params![snapshot.identity.device_key,serde_json::to_string(snapshot).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn snapshot(&self,key:&str)->Result<Option<DeviceSnapshot>,String> {
        let db=self.open()?;
        let value=db.query_row("SELECT payload FROM snapshots WHERE device_key=?1",[key],|r|r.get::<_,String>(0));
        match value { Ok(s)=>serde_json::from_str(&s).map(Some).map_err(|e|e.to_string()),Err(rusqlite::Error::QueryReturnedNoRows)=>Ok(None),Err(e)=>Err(e.to_string()) }
    }
    /// Save once, before any optimization. Later scans cannot replace this baseline.
    pub fn ensure_baseline(&self,snapshot:&DeviceSnapshot)->Result<DeviceSnapshot,String> {
        let mut db=self.open()?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        tx.execute("INSERT OR IGNORE INTO baselines(device_key,payload) VALUES(?1,?2)",params![snapshot.identity.device_key,serde_json::to_string(snapshot).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        let payload:String=tx.query_row("SELECT payload FROM baselines WHERE device_key=?1",[&snapshot.identity.device_key],|r|r.get(0)).map_err(|e|e.to_string())?;
        let original=serde_json::from_str(&payload).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        Ok(original)
    }
    pub fn baseline(&self,key:&str)->Result<Option<DeviceSnapshot>,String> {
        let payload:Option<String>=self.open()?.query_row("SELECT payload FROM baselines WHERE device_key=?1",[key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        payload.map(|p|serde_json::from_str(&p).map_err(|e|e.to_string())).transpose()
    }
    pub fn baseline_changes(&self,key:&str)->Result<Vec<ChangeRecord>,String> {
        let db=self.open()?;
        let mut st=db.prepare("SELECT c.payload FROM changes c INNER JOIN baseline_members m ON m.change_id=c.id AND m.device_key=c.device_key WHERE m.device_key=?1 ORDER BY c.created_at DESC,c.rowid DESC").map_err(|e|e.to_string())?;
        let rows=st.query_map([key],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        rows.map(|r|r.map_err(|e|e.to_string()).and_then(|s|serde_json::from_str(&s).map_err(|e|e.to_string()))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(key:&str,at:i64)->DeviceSnapshot {DeviceSnapshot{identity:DeviceIdentity{serial:"ip".into(),device_key:key.into(),model:"model".into(),manufacturer:"maker".into(),android_version:"9".into(),api:28,abi:"arm".into(),fingerprint:"firmware".into(),user_id:0},scanned_at:at,profile:None,packages:vec![],home_component:None,settings:Default::default(),memory_total_kb:None,memory_available_kb:None,storage_free_kb:None,processes:String::new(),inspections:vec![]}}
    fn record(id:&str,key:&str,at:i64)->ChangeRecord {ChangeRecord{id:id.into(),session_id:"session".into(),device_key:key.into(),serial:"ip".into(),model:"model".into(),fingerprint:"firmware".into(),user_id:0,kind:"dns".into(),target:"".into(),label:"DNS".into(),before_json:"{}".into(),after_json:"{}".into(),status:"prepared".into(),restore_status:"available".into(),error:None,created_at:at,updated_at:at}}
    fn cleanup(path:PathBuf){assert_eq!(path.parent(),Some(std::env::temp_dir().as_path()));assert!(path.file_name().unwrap().to_string_lossy().starts_with("tvopt-journal-"));let _=std::fs::remove_dir_all(path);}
    #[test]fn starting_snapshot_is_immutable_and_device_specific(){
        let path=std::env::temp_dir().join(format!("tvopt-journal-{}",uuid::Uuid::new_v4()));let j=Journal::new(&path).unwrap();
        let first=snapshot("tv",100);assert_eq!(j.ensure_baseline(&first).unwrap().scanned_at,100);
        let mut later=snapshot("tv",200);later.settings.insert("global/private_dns_mode".into(),Some("hostname".into()));j.save_snapshot(&later).unwrap();assert_eq!(j.ensure_baseline(&later).unwrap().scanned_at,100);
        j.ensure_baseline(&snapshot("other",300)).unwrap();drop(j);let j=Journal::new(&path).unwrap();let baseline=j.baseline("tv").unwrap().unwrap();assert_eq!(baseline.scanned_at,100);assert!(baseline.settings.is_empty());assert_eq!(j.snapshot("tv").unwrap().unwrap().scanned_at,200);assert_eq!(j.baseline("other").unwrap().unwrap().scanned_at,300);assert!(j.baseline("missing").unwrap().is_none());drop(j);cleanup(path);
    }
    #[test]fn restore_all_members_exclude_imports_and_prebaseline_records(){
        let path=std::env::temp_dir().join(format!("tvopt-journal-{}",uuid::Uuid::new_v4()));let j=Journal::new(&path).unwrap();
        let mut old=record("before-baseline","tv",1);j.save(&old).unwrap();j.ensure_baseline(&snapshot("tv",100)).unwrap();old.updated_at=200;j.save(&old).unwrap();j.save(&record("timestamp-before","tv",99)).unwrap();
        let mut imported=record("imported","tv",110);j.save_imported(&imported).unwrap();imported.status="verified".into();j.save(&imported).unwrap();
        j.save(&record("local-one","tv",120)).unwrap();j.save(&record("local-two","tv",130)).unwrap();j.save(&record("local-tied","tv",130)).unwrap();j.save(&record("other","other",140)).unwrap();
        let mut update=record("local-one","tv",120);update.status="verified".into();update.updated_at=150;j.save(&update).unwrap();let local=j.baseline_changes("tv").unwrap();assert_eq!(local.iter().map(|r|r.id.as_str()).collect::<Vec<_>>(),vec!["local-tied","local-two","local-one"]);assert_eq!(local.last().unwrap().status,"verified");assert_eq!(j.list(Some("tv")).unwrap().len(),6);assert!(j.baseline_changes("other").unwrap().is_empty());drop(j);cleanup(path);
    }
    #[test]
    fn history_survives_reopening_and_keeps_individual_records() {
        let path=std::env::temp_dir().join(format!("tvopt-journal-{}",uuid::Uuid::new_v4()));
        let j=Journal::new(&path).unwrap();
        let r=ChangeRecord{id:"one".into(),session_id:"session".into(),device_key:"tv".into(),serial:"ip".into(),model:"model".into(),fingerprint:"firmware".into(),user_id:0,kind:"dns".into(),target:"".into(),label:"DNS".into(),before_json:"{\"mode\":null}".into(),after_json:"{}".into(),status:"prepared".into(),restore_status:"available".into(),error:None,created_at:1,updated_at:1};
        j.save(&r).unwrap(); drop(j);
        let j=Journal::new(&path).unwrap(); assert_eq!(j.get("one").unwrap().before_json,r.before_json);assert!(j.list(Some("other")).unwrap().is_empty());assert_eq!(j.list(None).unwrap().len(),1);
        drop(j); cleanup(path);
    }
}
