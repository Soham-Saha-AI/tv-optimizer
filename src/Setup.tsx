import {OnlineConsent} from './OnlineConsent';
import {useState} from 'react';
import {isDesktop} from './api';
import {deviceConnectionMessage} from './domain';
import {Icon} from './ui';
import type {Optimizer} from './useOptimizer';

export function Setup({app}:{app:Optimizer}) {
  const [address,setAddress]=useState('');
  const [pairAddress,setPairAddress]=useState('');
  const [code,setCode]=useState('');
  const [step,setStep]=useState(0);
  const [searched,setSearched]=useState(false);
  const findTV=()=>{setSearched(true);return app.discover()};
  const steps=[
    {title:'Put both devices on the same network',text:'Connect your TV and this PC to the same home Wi-Fi or router. Keep the TV on.',icon:'wifi'},
    {title:'Allow a trusted connection',text:'On Android TV open Settings → Device preferences → About. Select Build seven times. Return to Developer options and turn on USB debugging or Network debugging.',icon:'setup'},
    {title:'Accept the prompt on your TV',text:'When the TV asks “Allow USB debugging?”, use your remote to choose Allow. Then select Find my TV here. Only authorize your own PC.',icon:'check'},
  ];
  return <>
    <div className="page-heading"><div><h1>Let’s connect your TV.</h1><p>A few guided steps. No commands or accounts needed.</p></div></div>
    <OnlineConsent app={app}/>{app.scan&&!app.confirmedDevice?<section className="surface confirm-device" aria-label="Confirm the scanned TV"><Icon name="tv" size={32}/><div><h2>Is this your TV?</h2><p>{app.scan.snapshot.identity.model} · Android {app.scan.snapshot.identity.android_version}</p><small className="muted">The scan is complete. No TV settings have changed.</small></div><button className="primary" onClick={app.confirmDevice}>Yes, this is my TV</button></section>:null}
    <div className="setup-layout">
      <section className="surface">
        <div className="step-tabs" aria-label="Setup steps">{steps.map((item,index)=><button key={item.title} className={step===index?'active':''} aria-label={`Step ${index+1}: ${item.title}`} aria-current={step===index?'step':undefined} onClick={()=>setStep(index)}>0{index+1}</button>)}</div>
        <div className="setup-illustration"><Icon name={steps[step].icon} size={64}/><div className="tv-outline"><span>Android TV</span><i/></div></div>
        <h2>{steps[step].title}</h2><p>{steps[step].text}</p>
        <div className="inline-actions"><button disabled={step===0} onClick={()=>setStep(value=>value-1)}>Back</button><button className="primary" onClick={()=>step<2?setStep(value=>value+1):findTV()} disabled={!!app.busy}>{step<2?'Next step':'Find my TV'}<Icon name="arrow" size={16}/></button></div>
      </section>
      <div>
        <section className="surface">
          <h2>Find my TV</h2><p>Look for TVs on your Wi-Fi or connected to this PC by USB. You may need to allow the connection on your TV.</p>
          <button className="primary" onClick={findTV} disabled={!!app.busy}><Icon name="search" size={16}/>{app.busy==='Searching for TVs'?'Finding your TV…':'Find my TV'}</button>
          {app.devices.length>0?<ul className="device-list">{app.devices.map(device=><li key={device.serial}><div><strong>{device.model||'Android TV'}</strong><small>{deviceConnectionMessage(device.state)}</small><details><summary>Connection details</summary><small>{device.serial}</small></details></div><button disabled={!!app.busy||device.state!=='device'} onClick={()=>app.inspect(device.serial)}>{device.state==='unauthorized'?'Allow on TV first':device.state==='device'?'Scan this TV':'Not ready yet'}</button></li>)}</ul>:<p className="small muted">{searched?'No TV found yet. Check the TV’s permission prompt, try again, or enter its IP address below.':'Your TV will appear here. If it does not, enter its IP address below.'}</p>}
          {app.devices.some(device=>device.state==='unauthorized')?<p className="permission-help">On the TV, choose <strong>Allow</strong> on the permission prompt. Then select <strong>Find my TV</strong> again.</p>:null}
          <form onSubmit={event=>{event.preventDefault();app.connect(address.trim())}}>
            <label htmlFor="tv-address">Your TV’s IP address</label>
            <div className="input-action"><input id="tv-address" type="text" autoComplete="off" spellCheck={false} placeholder="192.0.2.10" value={address} onChange={event=>setAddress(event.target.value)} aria-describedby="tv-address-help tv-port-help" required/><button disabled={!address.trim()||!!app.busy}>{app.busy==='Connecting to your TV'?'Connecting…':'Connect to TV'}</button></div>
            <small id="tv-address-help">Find the IP address under TV Settings → Network. Enter the numbers exactly as shown.</small>
            <small id="tv-port-help">The usual connection port, 5555, is added automatically. If Wireless debugging shows a different connection port, paste that full address instead.</small>
          </form>
        </section>
        <details className="surface compact"><summary>My TV shows a pairing code</summary><form onSubmit={event=>{event.preventDefault();app.pair(pairAddress.trim(),code.trim())}}>
          <p>Some newer TVs ask you to pair first. Open Wireless debugging → Pair device with pairing code on your TV, then copy the pairing address and code shown there.</p>
          <label htmlFor="pair-address">Pairing IP address and port</label><input id="pair-address" type="text" autoComplete="off" spellCheck={false} value={pairAddress} placeholder="192.0.2.10:37123" onChange={event=>setPairAddress(event.target.value)} aria-describedby="pair-port-help" required/>
          <small id="pair-port-help">Include the pairing port after the colon. It is different from the connection port, and is not filled in automatically.</small>
          <label htmlFor="pair-code">Six-digit pairing code</label><input id="pair-code" value={code} autoComplete="off" inputMode="numeric" pattern="[0-9]{6}" maxLength={6} onChange={event=>setCode(event.target.value)} required/>
          <button disabled={!!app.busy} className="primary">Pair TV</button>
          <p className="small pairing-next"><strong>After pairing:</strong> return to the TV’s main Wireless debugging screen. Copy its separate connection IP address and port into the Connect to TV field above.</p>
        </form></details>
      </div>
    </div>
    <div className="setup-foot"><p><Icon name="info" size={18}/>Supports compatible Android TV devices. Samsung Tizen and LG webOS are not supported.</p>{!isDesktop()?<p className="desktop-note">You are viewing the app in a browser. Use the portable Windows application to connect your real TV.</p>:null}<button className="text-button" disabled={!!app.busy} onClick={app.openPreview}>Explore with sample data<Icon name="arrow" size={16}/></button></div>
  </>;
}

