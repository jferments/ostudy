# Android release signing

`release-certificate.sha256` is the SHA-256 fingerprint of the certificate that
signs every public oStudy APK. `scripts/sign-android-release.sh` refuses to
publish an APK signed by any other key, because Android would refuse it as an
update. The fingerprint is public; the keystore and its password stay outside
the repository. See [Android: the public APK](../../../docs/installation/android.md#the-public-apk).
