import {useEffect,useRef,useState} from 'react';
import {actionKey} from './domain';
import {recommendationItem} from './Overview';
import {Icon,Status} from './ui';
import type {Optimizer} from './useOptimizer';

const tiers=[
  {id:'adguard',title:'AdGuard',subtitle:'An easy starting point',description:'Everyday ad and tracker filtering, with less chance of interrupting services.',warning:'Some ads remain, especially ads inside videos.',host:'dns.adguard-dns.com',isPublicFilter:true},
  {id:'pro',title:'HaGeZi Pro',subtitle:'Balanced privacy',description:'Broader tracker and advertising protection for everyday TV use.',warning:'Test your usual streaming and sign-in apps after applying.',host:'x-hagezi-pro.freedns.controld.com',isPublicFilter:true},
  {id:'proplus',title:'HaGeZi Pro++',subtitle:'Stronger filtering',description:'A stricter choice for people willing to check and restore individual changes.',warning:'May disrupt sign-in, video content, or streaming services.',host:'x-hagezi-proplus.freedns.controld.com',isPublicFilter:true},
  {id:'ultimate',title:'HaGeZi Ultimate',subtitle:'Aggressive protection',description:'The most restrictive option here. Choose only if you can test every app you use.',warning:'High chance of breakage. Some apps, logins, and content may stop working.',host:'x-hagezi-ultimate.freedns.controld.com',isPublicFilter:true},
  {id:'custom',title:'Custom Private DNS (NextDNS / Control D)',subtitle:'Supports your own allowlist',description:'Use your personal NextDNS, Control D, or AdGuard DNS hostname so you can allowlist JioHotstar domains while blocking ads everywhere else.',warning:'Requires your personal Private DNS hostname (no http:// or slashes).',host:'',isPublicFilter:false},
  {id:'default',title:'Default TV DNS (Turn off Private DNS)',subtitle:'Factory / system default',description:'Clears custom Private DNS overrides so your TV uses its standard network and router DNS settings.',warning:'Turns off DNS-level ad and tracker filtering.',host:'',isPublicFilter:false}
];

const JIOHOTSTAR_ALLOWLIST=[
  'bifrost-api.hotstar.com',
  'hesads.akamaized.net',
  'hspx.hotstar.com',
  'api.hotstar.com',
  'apix.hotstar.com',
  'service.hotstar.com',
  'livingroom.hotstar.com'
];

function isValidCustomHost(value:string){
  const host=value.trim().toLowerCase();
  if(host.length<3||host.length>253||!host.includes('.')||!/[a-z]/.test(host))return false;
  return host.split('.').every(label=>label.length>0&&label.length<=63&&!label.startsWith('-')&&!label.endsWith('-')&&/^[a-z0-9-]+$/.test(label));
}

export function Ads({app}:{app:Optimizer}){
  const snapshot=app.scan!.snapshot;
  const deviceKey=snapshot.identity.device_key;
  const guardKey=`tv-optimizer.jiohotstar-guard:${deviceKey}`;
  const customHostKey=`tv-optimizer.custom-dns-host:${deviceKey}`;
  const recs=app.scan!.recommendations;
  const settings=snapshot.settings;
  const hasJioHotstar=snapshot.packages.some(pkg=>pkg.installed&&(pkg.id==='in.startv.hotstar'||pkg.id==='com.jio.media.ondemand'));
  const dnsKnown='global/private_dns_mode' in settings;
  const mode=settings['global/private_dns_mode'];
  const hostname=settings['global/private_dns_specifier'];
  const activeCustomHost=mode==='hostname'&&hostname&&!tiers.some(item=>item.isPublicFilter&&item.host===hostname.toLowerCase())?hostname.toLowerCase():'';

  const readGuard=()=>{
    try{
      const saved=localStorage.getItem(guardKey);
      if(saved==='on')return true;
      if(saved==='off')return false;
    }catch{}
    return hasJioHotstar;
  };
  const readCustomHost=()=>{
    try{return activeCustomHost||localStorage.getItem(customHostKey)||'';}catch{return activeCustomHost;}
  };

  const [tier,setTier]=useState('');
  const [ack,setAck]=useState(false);
  const [copyStatus,setCopyStatus]=useState<'idle'|'copied'|'failed'>('idle');
  const copyTimer=useRef<number|null>(null);
  const [customHost,setCustomHost]=useState(readCustomHost);
  const [guardOn,setGuardOn]=useState(readGuard);

  useEffect(()=>{
    setTier('');
    setAck(false);
    setGuardOn(readGuard());
    setCustomHost(readCustomHost());
  },[deviceKey,activeCustomHost,hasJioHotstar]);

  useEffect(()=>()=>{if(copyTimer.current!==null)window.clearTimeout(copyTimer.current);},[]);

  const toggleGuard=(next:boolean)=>{
    setGuardOn(next);
    try{localStorage.setItem(guardKey,next?'on':'off');}catch{}
  };

  const normalizedHost=hostname?.toLowerCase()??'';
  const current=!dnsKnown?'':mode==='hostname'?tiers.find(item=>item.host===normalizedHost)?.title??`Custom (${hostname})`:hostname?`Custom override (${hostname})`:mode==='off'?'Off (non-default)':mode?`System DNS (${mode})`:'Default TV DNS (System)';
  const selectedTier=tiers.find(item=>item.id===tier);
  const selected=recs.find(rec=>rec.action.kind==='dns'&&rec.action.value===tier);
  const defaultRec=recs.find(rec=>rec.action.kind==='dns'&&rec.action.value==='default');
  const normalizedCustom=customHost.trim().toLowerCase();
  const customValid=isValidCustomHost(normalizedCustom);
  const customIsBundledFilter=tiers.some(item=>item.isPublicFilter&&item.host===normalizedCustom);
  const customMatchesCurrent=mode==='hostname'&&normalizedHost===normalizedCustom;
  const guardBlocked=guardOn&&(Boolean(selectedTier?.isPublicFilter)||(tier==='custom'&&customIsBundledFilter));
  const canApply=tier==='custom'
    ?Boolean(selected?.supported&&customValid&&!customMatchesCurrent&&!guardBlocked&&!app.busy)
    :Boolean(selected?.supported&&!selected?.already_applied&&!guardBlocked&&(tier!=='ultimate'||ack)&&!app.busy);

  const applySelection=()=>{
    if(!selected||guardBlocked)return;
    if(tier==='custom'){
      if(!customValid)return;
      try{localStorage.setItem(customHostKey,normalizedCustom);}catch{}
      app.reviewSelection({
        ...recommendationItem(selected),
        title:`Use custom Private DNS (${normalizedCustom})`,
        action:{kind:'dns',target:'private_dns',value:normalizedCustom}
      });
      return;
    }
    app.reviewSelection(recommendationItem(selected));
  };

  const copyAllowlist=async()=>{
    if(copyTimer.current!==null)window.clearTimeout(copyTimer.current);
    try{
      await navigator.clipboard.writeText(JIOHOTSTAR_ALLOWLIST.join('\n'));
      setCopyStatus('copied');
    }catch{
      setCopyStatus('failed');
    }
    copyTimer.current=window.setTimeout(()=>setCopyStatus('idle'),2500);
  };

  const reporting=recs.filter(rec=>rec.action.kind==='disable'&&(/privacy/i.test(rec.area)||/analytics|feedback|bugreport|telemetry|advertis/i.test(rec.title)));

  return <>
    <div className="page-heading">
      <div>
        <h1>A little less noise.</h1>
        <p>Choose ad protection, turn Private DNS off anytime, and avoid JioHotstar locks.</p>
      </div>
    </div>

    <div className="info-banner">
      <Icon name="ads"/>
      <p>DNS can block known advertising and tracking domains. It cannot remove every sponsored section or ads served alongside video content, including many YouTube ads.</p>
    </div>

    <section className="section-block">
      <div className="section-title">
        <h2>Choose your DNS protection</h2>
        <span className="small muted">Current: {current||'Could not check'}</span>
      </div>
      <p className="section-description">Choose filtering on, custom DNS, or turn Private DNS back off to default TV settings. Nothing changes until you review and apply.</p>

      <div className="setting-row">
        <label className="setting-select">
          <input type="checkbox" checked={guardOn} onChange={event=>toggleGuard(event.target.checked)} disabled={!!app.busy}/>
          <span>
            <strong>JioHotstar Safe Guard {hasJioHotstar?'(JioHotstar detected on TV)':''}</strong>
            <p>Prevents accidental use of public AdGuard or HaGeZi filters that block JioHotstar ad servers and can trigger a 24-hour account lock (NM-1001_429 / PB_ANDROID_NM-4290).</p>
            <small>{guardOn?'ON: Use Default TV DNS (Off) when watching JioHotstar, or use Custom Private DNS with the allowlist below. Turn this off if you still want a public filter.':'OFF: All public DNS options are unlocked.'}</small>
          </span>
        </label>
        <Status status={guardOn?'already_optimized':'detected'}/>
      </div>

      {defaultRec?.supported&&!defaultRec.already_applied?<div className="callout">
        {mode==='hostname'
          ?<><strong>Private DNS is currently on ({current}).</strong> Want plain TV/router DNS right now?</>
          :<><strong>Private DNS has an active setting ({current}).</strong> Want Android’s factory Automatic default instead?</>}
        <div className="inline-actions" style={{marginTop:'10px'}}>
          <button className="primary" disabled={!!app.busy} onClick={()=>app.reviewSelection(recommendationItem(defaultRec))}>Turn Private DNS off (Default TV)</button>
        </div>
      </div>:null}

      <div className="dns-list">
        {tiers.map(item=>{
          const rec=recs.find(value=>value.action.kind==='dns'&&value.action.value===item.id);
          return <label className={`dns-row ${tier===item.id?'selected':''}`} key={item.id}>
            <input type="radio" name="dns-tier" value={item.id} checked={tier===item.id} onChange={()=>{setTier(item.id);setAck(false)}} disabled={!rec?.supported||!!app.busy}/>
            <div>
              <div className="inline-title">
                <h3>{item.title}</h3>
                <span className="muted small">{item.subtitle}</span>
                {rec?.already_applied?<Status status="already_optimized"/>:null}
              </div>
              <p>{item.description}</p>
              <small className={item.id==='ultimate'?'caution-text':''}>{item.warning}</small>
              {guardOn&&item.isPublicFilter?<small className="caution-text">Blocked while JioHotstar Safe Guard is ON (public filters cannot allowlist JioHotstar domains).</small>:null}
              {!rec?.supported?<small className="caution-text">Private DNS or this TV’s permission could not be verified. Android 9+ is required.</small>:null}
            </div>
          </label>;
        })}
      </div>

      {tier==='custom'?<div className="surface" style={{marginTop:'14px'}}>
        <label htmlFor="custom-dns-host"><strong>Your personal Private DNS hostname</strong></label>
        <div className="input-action" style={{marginTop:'8px'}}>
          <input id="custom-dns-host" type="text" autoComplete="off" spellCheck={false} placeholder="abc123.dns.nextdns.io" value={customHost} onChange={event=>setCustomHost(event.target.value)} disabled={!!app.busy}/>
        </div>
        <small className={(normalizedCustom&&!customValid)||(guardOn&&customIsBundledFilter)?'caution-text':'muted'}>
          {normalizedCustom&&!customValid?'Enter only a valid hostname like abc123.dns.nextdns.io (no spaces, http://, ports, or IP addresses).':guardOn&&customIsBundledFilter?'That hostname points to a public non-customizable filter, which is blocked while JioHotstar Safe Guard is ON.':'Paste your NextDNS, Control D, or personal AdGuard DNS DoT hostname.'}
        </small>
      </div>:null}

      <details className="surface compact" style={{marginTop:'14px'}}>
        <summary>JioHotstar allowlist domains (for NextDNS / Control D / router AdGuard)</summary>
        <p className="small muted" style={{marginTop:'8px'}}>Add these domains to your personal DNS Allowlist so JioHotstar ad/heartbeat checks succeed without triggering a 24-hour ban while other apps remain filtered:</p>
        <pre style={{margin:'8px 0'}}>{JIOHOTSTAR_ALLOWLIST.join('\n')}</pre>
        <div className="inline-actions">
          <button type="button" onClick={copyAllowlist}>{copyStatus==='copied'?'Copied allowlist!':'Copy allowlist domains'}</button>
          {copyStatus==='failed'?<small className="caution-text">Clipboard access was blocked — select the domains above and press Ctrl+C.</small>:null}
        </div>
      </details>

      {tier==='ultimate'&&!guardBlocked?<label className="check-explanation caution"><input type="checkbox" checked={ack} onChange={event=>setAck(event.target.checked)}/><span><strong>I understand some apps may stop working</strong><small>I will test sign-in and playback, and restore the previous DNS if needed.</small></span></label>:null}

      <div className="inline-actions" style={{marginTop:'14px'}}>
        <button className="primary" disabled={!canApply} onClick={applySelection}>
          {guardBlocked?'Blocked by JioHotstar Safe Guard':tier==='default'?(selected?.already_applied?'Already at default':'Review & restore default DNS'):tier==='custom'?(customMatchesCurrent?'Already active':'Review & apply custom DNS'):selected?.already_applied?'Already active':'Review & apply protection'}
        </button>
        <button onClick={()=>app.setPage('restore')}>Restore a previous DNS setting</button>
      </div>

      <p className="footnote">Connectivity is checked before keeping a change. Public providers do not offer a per-TV allowlist. Your original DNS mode and hostname are saved together.</p>
    </section>

    <section className="section-block">
      <h2>Usage, diagnostics, and advertising services</h2>
      <p className="section-description">Only known optional services appear. Choose each service individually.</p>
      {reporting.length?reporting.map(rec=>{
        const active=app.basket.some(item=>actionKey(item.action)===actionKey(rec.action));
        return <div className="setting-row" key={rec.id}><label className="setting-select"><input type="checkbox" checked={active} disabled={!rec.supported||rec.already_applied||!!app.busy} onChange={()=>app.toggle(recommendationItem(rec))}/><span><strong>{rec.title}</strong><p>{rec.description}</p><small>{rec.impact}</small></span></label>{rec.already_applied?<Status status="already_optimized"/>:null}</div>;
      }):<p className="empty-inline">No known optional reporting services were detected with a supported change.</p>}
    </section>

    <div className="setting-row">
      <div>
        <strong>Remove recommendations from Home</strong>
        <p>SwiftOS and Original CleanYourTV offer a clean Home. Google Home and PatchWall can be kept or disabled independently after confirming a replacement.</p>
      </div>
      <button onClick={()=>app.setPage('launchers')}>Manage Home screens</button>
    </div>
  </>;
}
