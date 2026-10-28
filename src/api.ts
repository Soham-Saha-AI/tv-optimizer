import type {OriginalStatus} from './OriginalLauncher';
import type {CommunityStatus} from './CommunityUpdates';
import type {ActionRequest,ApplyResult,ChangeRecord,DeviceRef,OperationProgress,PlanPreview,RuntimeInfo,ScanResult,OnlineCompatibility,RestoreAllPreview} from './models';
export const isDesktop=()=>typeof window!=='undefined'&&'__TAURI_INTERNALS__' in window;
async function command<T>(name:string,args?:Record<string,unknown>):Promise<T>{if(!isDesktop())throw new Error('Open the portable Windows application to connect to a TV. This browser cannot run TV commands.');const {invoke}=await import('@tauri-apps/api/core');return invoke<T>(name,args)}
export const api={
 originalStatus:()=>command<OriginalStatus>('original_launcher_status'),
 prepareOriginal:(serial:string,acceptSdkTerms:boolean)=>command<OriginalStatus>('prepare_original_launcher',{serial,acceptSdkTerms}),
 openSdkTerms:()=>command<void>('open_sdk_terms'),
 communityStatus:(model:string,refresh:boolean)=>command<CommunityStatus>('community_status',{model,refresh}),
 importHistoryData:(serial:string,json:string)=>command<number>('import_history_data',{serial,json}),
 onlineCompatibility:(serial:string,officialUrl?:string,includeManual=false)=>command<OnlineCompatibility>('online_compatibility',{serial,officialUrl:officialUrl||null,includeManual}),
 runtime:()=>command<RuntimeInfo>('runtime_info'),devices:()=>command<DeviceRef[]>('list_devices'),discover:()=>command<DeviceRef[]>('discover_devices'),
 connect:(address:string)=>command<string>('connect_device',{address}),pair:(address:string,code:string)=>command<string>('pair_device',{address,code}),
 scan:(serial:string)=>command<ScanResult>('scan_device',{serial}),preview:(serial:string,actions:ActionRequest[])=>command<PlanPreview>('preview_plan',{serial,actions}),
 apply:(serial:string,plan:PlanPreview)=>command<ApplyResult>('apply_plan',{serial,plan}),confirmHome:(serial:string,token:string)=>command<ChangeRecord>('confirm_home',{serial,token}),
 history:(deviceKey?:string)=>command<ChangeRecord[]>('history',{deviceKey:deviceKey??null}),restore:(serial:string,changeId:string)=>command<ChangeRecord>('restore_change',{serial,changeId}),
 restoreSession:(serial:string,sessionId:string)=>command<ChangeRecord[]>('restore_session',{serial,sessionId}),
 restoreAllPreview:(serial:string)=>command<RestoreAllPreview>('preview_restore_all',{serial}),restoreAll:(serial:string,plan:RestoreAllPreview)=>command<ChangeRecord[]>('restore_all',{serial,plan}),
 cancel:()=>command<void>('cancel_operations'),exportReport:(deviceKey:string)=>command<string>('export_report',{deviceKey}),importHistory:(serial:string,path:string)=>command<number>('import_history',{serial,path})
};
export async function listenProgress(callback:(event:OperationProgress)=>void){if(!isDesktop())return ()=>{};const {listen}=await import('@tauri-apps/api/event');return listen<OperationProgress>('operation-progress',event=>callback(event.payload))}


