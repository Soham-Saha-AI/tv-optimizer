# SwiftOS v3 build23 — retained tested release

The newer optional local-wallpaper update is SwiftOS 3.0.1 build24. See `WALLPAPER-ENGINE.md` and `UPDATE-2026-10-27.md`. The build23 APK described below remains unchanged and separately available. Windows compilation is still blocked; the native source now pins build24 for a future build. The remainder of this document records build23's historical validation.

Current Android release: `com.gathin.tvhome`, `3.0-swiftos`, version code 23.
APK SHA-256: `AEE129904809BEF86F258D8873BBBA5EE7297986B61C2E0572CB9786EC7C4AEC`.
Size: 3,511,343 bytes (19.64% smaller than historical 0C; about 13.69% larger than build22).

This selective SwiftOS 2.7 derivative preserves the approved design, all five themes, languages, fonts, logos and opacity choices. Home and Library retain 12% app enlargement, 6% header and 4% control enlargement. Shared motion and navigation behavior remain preserved. It includes the Library-reveal fix, catalogue workers, bounded artwork caches, exact lossless PNGs, and guarded hardware artwork storage. It is not a full upstream CleanYourTV v3 rebase; Control Centre remains deferred.

The final optimization composites backgrounds in place at 25%, 50% and 75% icon-background opacity. Native proof passes 225 exact ARGB/raw comparisons. Isolated cold composition used 12.49% less CPU and 33.3% less transient bitmap allocation; those are operation-level measurements, not whole-app gains. The 0%, 100%, other percentages, fallback paths, hardware guards, motion and assets are preserved; 67 other Java source files are byte-identical to 7EF. Release checks passed 253 tests, lint, R8, signer continuity and asset guards. Earlier native hardware/zero-opacity/PNG proofs passed 130/75/156 comparisons.

Native-heap reductions were consistent in the tested hardware route. Reported PSS varied with private-clean APK mappings, including a higher first-open Library peak, so prior PSS percentages are historical samples and are not a general RAM guarantee. TV memtrack does not provide complete GPU accounting. No reliable general startup or navigation improvement is claimed.  

Auto wallpaper uses separately provisioned `clear.jpg`, `cloud.jpg`, `rain.jpg`, and `night.jpg` under `/sdcard/Pictures/SwiftOS/Weather/` as its primary lookup. Public-photo selection and Auto were verified through the TV GUI. The legacy external-files `auto-wallpapers/` directory is fallback only. The four public photos are outside the APK and release ZIP; their earlier relocation did not shrink this APK. Missing/stale/unsupported weather or unreadable photos preserve the current wallpaper.

`releases/SwiftOS-v3-TV-build23.zip` is a TV-only artifact with the current APK and protected build22 APK for rollback. It contains no Windows executable. See `design/swiftos-v3-build23/README.md` for historical and final device proof.

The existing full Windows portable ZIP remains the historical 0C release. Windows Application Control blocked Cargo before native compilation; the user chose to leave Windows security unchanged. No repair, restart, compiler retry, policy workaround or Windows packaging will be performed. The candidate Rust hash is pinned for a future approved build, but there is no completed AEE Windows GUI bundle. Existing receipt gates remain enforced; descriptive candidate metadata never authorizes APK bytes.

Final AEE TV validation passed: installed APK hash matches this release. Home, Library, Inputs and Settings passed visual review; first and last Settings selection borders are complete. Liquid Glass / 50% opacity and Auto-off state were preserved. Public Auto rain-photo selection and restoration passed.  Three cold first-display samples were 866 / 849 / 807 ms, without an overall speed-gain claim. Runtime-error scan contained headers only. Motion and assets remain preserved; the matching motion was reviewed. Long-duration use and physical held-key/double Circle behavior remain outside this bounded validation.

Final TV-only ZIP: `releases/SwiftOS-v3-TV-build23.zip`, 5839844 bytes, SHA-256 `6a197148f174bcfd659414b162d7a9678c1a4316a39c5199af04257f9138f30c`. All four entries and checksum lines were read back and verified. See `final-TV-release-receipt.json` and `final-TV-device-validation.json` in the build23 evidence directory. The Windows 0C ZIP remains unchanged.
