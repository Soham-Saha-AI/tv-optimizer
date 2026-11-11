# Local Wallpaper Engine — SwiftOS v3 build 25

This update is tested on one Android TV. PC wallpaper management targets Android user 0 explicitly; other TV users are not yet supported. It is not a webOS launcher. SwiftOS 2.7 build 22 and the previously tested v3 build 23 remain separate, untouched APKs. All versions share the `com.gathin.tvhome` app identity; only one version can be installed at a time. Compatible updates preserve launcher preferences. Android blocks ordinary downgrades; do not uninstall SwiftOS to force one unless you accept losing its private settings.

## Add your photos

[TV Optimizer](https://github.com/Soham-Saha-AI/tv-optimizer) includes a **Wallpaper Engine** tab. Choose weather and time, select photos, then **Prepare & review transfer**. Confirm **Transfer photos**. This prepares photos on the PC and copies them to the TV; it does not turn automatic wallpaper on.

Use **Review turning on**, check the explanation, then **Apply**. Press Home on the TV. If SwiftOS needs photo access, allow it in its TV settings first. You can also use SwiftOS's existing **Auto wallpaper** option directly on the TV.

## Copy directly on the TV

The shared storage folder is named **Auto Wallpaper**. Folder names inside it use the following lowercase words:

| Weather folder | Use |
| --- | --- |
| `clear` | Clear or mainly clear |
| `cloudy` | Partly cloudy or overcast |
| `fog` | Fog or mist |
| `drizzle` | Drizzle, including freezing drizzle |
| `rain` | Rain, freezing rain or rain showers |
| `snow` | Snow, grains or snow showers |
| `thunderstorm` | Thunderstorms, including hail |
| `unknown` | Weather is unavailable, stale or unsupported |
| `any` | Fallback for any weather |

Each weather folder can contain `morning`, `day`, `evening`, `night` and `all` folders. The TV's local clock sets the period: morning 06:00–10:00, day 10:00–17:00, evening 17:00–21:00, night 21:00–06:00.

For example, put a rainy evening photo in **Auto Wallpaper → rain → evening**. Put general fallback photos in **Auto Wallpaper → any → all**. Photos directly in Auto Wallpaper also act as any-weather/all-time fallbacks.

Selection checks weather + time, weather + all, any + time, then any + all. Multiple photos in a group use a 30-minute selection slot; the visible change occurs at an existing weather refresh, a time-period boundary, or the next visit to Home. There is no extra 30-minute polling service. When no matching local photo exists, SwiftOS checks its earlier weather-photo folders and otherwise keeps the current wallpaper.

All documented Open-Meteo weather codes map to a weather family; unknown or stale weather falls back to `unknown` / `any` photos.

## Cold-boot theme and opacity persistence (build 25)

On-TV theme selections (`ThemePicker`) and icon background opacity settings commit synchronously via `.commit()`, keep fallback copies of `config.json` and `wallpaper.jpg` in private internal storage, and never wipe user customizations during cold boot when external storage mounts asynchronously.

## Resource limits

- Still JPG, JPEG, PNG and WebP only. No video playback or wallpaper service.
- PC transfers prepare JPEGs at up to 1920 × 1080, preserving proportions, with at most 2 MB per prepared image, 30 images and 20 MB per batch.
- Direct TV files must be at most 20 MB each. Keep the collection below 256 images. Directory inspection and indexing are bounded and run on the existing decoder worker.
- One photo is decoded when selection changes. The existing sampled decoder bounds resolution. Visible work reuses the weather data; new work stops while Home is hidden. An in-flight decode may finish and be discarded.
- The active photo consumes bitmap memory. One measured Mi TV load used 4,147,200 bitmap bytes, 389 ms elapsed and 163 ms worker CPU time. These are single-load samples, not a general responsiveness or memory guarantee.
- A hidden-Home sample fell from about 31.4 MiB to 21.6 MiB PSS after releasing the automatic image. PSS varies and the TV does not report complete graphics memory. Long-duration playback and physical remote testing remain pending.

Uploaded photos remain on the TV when you turn Auto off or restore its enabled setting. Restore does not delete a photo collection.

## Verification

Build 25 (`3.0.2-swiftos`, `versionCode 25`) passed **260** Android JVM tests, release lint, R8, signer continuity (`e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986`) and 156 pixel-verified artwork integrity checks.

The underlying cold-reboot persistence fix and App Library optimization were previously installed and verified on one Android TV as build 24 (`3.0.1-swiftos`) on 2 November 2026, alongside earlier bounded Mi TV local-wallpaper smoke checks (photo permission, cloudy/evening selection, generic fallback, and restoring Auto wallpaper to Off). The renumbered build 25 APK passed all offline build and JVM unit-test gates and has not yet been re-installed onto hardware.

Build 25 SHA-256: `c3209b1194650f05056ea40ac4b1ed66a300249e3cae81f34de1036778b50859`.
