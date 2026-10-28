# Local Wallpaper Engine — SwiftOS v3 build 24

This update is tested on one Android TV. PC wallpaper management targets Android user 0 explicitly; other TV users are not yet supported. It is not a webOS launcher. SwiftOS 2.7 build 22 and the previously tested v3 build 23 remain separate, untouched APKs. All three use the same app identity; only one version can be installed. A compatible update preserves launcher preferences. Android blocks ordinary downgrades; do not uninstall SwiftOS to force one unless you accept losing its private settings.

## Add your photos

The updated Windows interface has a **Wallpaper Engine** tab. Choose weather and time, select photos, then **Prepare & review transfer**. Confirm **Transfer photos**. This prepares photos on the PC and copies them to the TV; it does not turn automatic wallpaper on.

Use **Review turning on**, check the explanation, then **Apply**. Press Home on the TV. If SwiftOS needs photo access, allow it in its TV settings first. You can also use SwiftOS's existing **Auto wallpaper** option directly on the TV.

**Windows delivery status, 28 October:** the native release builds successfully and passes 63 native tests. The updated portable package includes this interface and native integration. No Windows security setting was changed. The earlier compilation block is resolved for this build. Further TV tests were not requested for this package update; previous bounded wallpaper checks remain recorded below.

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

This build reuses SwiftOS's existing **Kolkata weather** configuration. It does not infer your location or add new weather requests. All documented forecast codes map to a weather family; unknown values use fallback photos.

## Resource limits

- Still JPG, JPEG, PNG and WebP only. No video playback or wallpaper service.
- PC transfers prepare JPEGs at up to 1920 × 1080, preserving proportions, with at most 2 MB per prepared image, 30 images and 20 MB per batch.
- Direct TV files must be at most 20 MB each. Keep the collection below 256 images. Directory inspection and indexing are bounded and run on the existing decoder worker.
- One photo is decoded when selection changes. The existing sampled decoder bounds resolution. Visible work reuses the weather data; new work stops while Home is hidden. An in-flight decode may finish and be discarded.
- The active photo consumes bitmap memory. One measured Mi TV load used 4,147,200 bitmap bytes, 389 ms elapsed and 163 ms worker CPU time. These are single-load samples, not a general responsiveness or memory guarantee.
- A hidden-Home sample fell from about 31.4 MiB to 21.6 MiB PSS after releasing the automatic image. PSS varies and the TV does not report complete graphics memory. Long-duration playback and physical remote testing remain pending.

Uploaded photos remain on the TV when you turn Auto off or restore its enabled setting. Restore does not delete a photo collection. The two temporary test photos were removed and Auto was returned to its original Off state after validation.

## Verification

Build 24 passed 257 Android JVM tests, release lint, R8, signer continuity and artwork integrity checks. On the Mi TV, the installed APK hash matched the release, photo permission was already granted, cloudy/evening selection and generic fallback displayed correctly, and the provider returned the expected On/Off state. SwiftOS's process-specific runtime error check was clear during the bounded test. Weather families and time boundaries are also covered by unit tests; every real weather condition was not physically reproduced.

Build 24 SHA-256: `0c86c2542294a5468204acf3bf79c468051d270b6997dde7dabdaf92454a572b`.
