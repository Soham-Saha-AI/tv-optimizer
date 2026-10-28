# webOS support research

Reviewed 24 October 2026. This is a compatibility assessment for the planned LG integration. The current Windows package controls Android TV through ADB; it does not yet manage webOS devices.

The product direction is **no root required**. Root-dependent projects below are research references and are excluded from the beginner workflow. webOS support will be offered by capability, not advertised as working on every webOS device.

## Current projects

| Project | Relevant capability | Access and compatibility boundary |
|---|---|---|
| [webOS Dev Manager 1.99.20](https://github.com/webosbrew/dev-manager-desktop/releases/tag/v1.99.20) | Windows GUI for device setup, app installation, files and device information without installing LG's SDK. | Supports developer-mode or rooted devices. Developer mode has its own permission and session limits. |
| [CleanYourTV-WebOS 1.5.16](https://github.com/lasharor/cleanyourtv-WebOS/releases/tag/v1.5.16) | Replacement Home, optional ad/telemetry service suppression, backup and launcher fallback. | Requires root/Homebrew Channel. Its [published compatibility file](https://github.com/lasharor/cleanyourtv-WebOS/blob/main/compat.json) lists firmware 33.23.05 as tested and 23.25.55 as reported; that does not establish support for a particular device. |
| [Glasshouse 0.118.0](https://github.com/rorygallagher2024/glasshouse/releases/tag/v0.118.0) | Dashboard, privacy controls, app management, ad/telemetry blocking and remote/Home features. | Requires root. Home and remote handling vary substantially by webOS generation; monitor support needs direct inspection. [Project documentation](https://github.com/rorygallagher2024/glasshouse). |
| [own-your-glass](https://github.com/JulioFerrero/own-your-glass) | Modular privacy and service controls with restoration. | Requires root. Some modules disable voice, phone-remote access, ThinQ or update services. These cannot be adopted as a blanket beginner default. |
| [youtube-webos](https://github.com/webosbrew/youtube-webos) | A separate YouTube app with ad-blocking enhancements. | App-specific, rather than a whole-system optimizer. Check its current device requirements and coexistence with the stock YouTube app. |

[LG's developer-mode instructions](https://webostv.developer.lge.com/develop/getting-started/developer-mode-app) document authorized app testing and session renewal. Developer mode does not provide the same permissions as root. [webOS Homebrew's comparison](https://www.webosbrew.org/rooting/) distinguishes app management from privileged remote and system customization.

## Integration approach

1. Complete the Android TV compatibility checks first.
2. Record each target device model, firmware, SDK version and remote type. Match its specifications and manual on LG's official website. The marketing label “webOS 23” alone is insufficient.
3. Add a separate webOS connection and capability adapter with the same scan, review, history and individual-restore interface. Begin with authorized information and ordinary app-management capabilities.
4. Exclude root-only Home takeover, privileged service suppression and firmware modifications from this app. Begin with supported developer-mode app management, official settings guidance and network filtering options that do not require root. A scan must never initiate rooting.
5. Review project code, license, release hash, startup behavior and rollback before adopting a component. A GitHub release is not proof that an operation works on the smart monitor.
6. Treat DNS as a platform-specific feature. Android Private DNS commands and provider hostnames cannot be copied into webOS settings. Assess the monitor's network settings and router-based filtering separately; preserve its original addressing and DNS configuration.
7. Test physical D-pad, OK, Back, Home, held volume, pointer/wheel if present, microphone, Input, HDMI, casting, streaming, standby and startup. Never mark absent TV-specific or OLED features as working on a monitor.

SwiftOS's existing APK remains an Android app. A webOS edition would be a separate IPK/web app with its own input, startup and recovery implementation; renaming an APK does not create webOS compatibility.

No third-party installer or rooting command was run, and no LG device has been modified during this research.
