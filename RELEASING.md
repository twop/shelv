## App Distribution

### Resources:

- [Creating Distribution-Signed Code for Mac](https://developer.apple.com/forums/thread/701514#701514021)
- [Packaging Mac Software for Distribution](https://developer.apple.com/forums/thread/701581#701581021)
- [Managing Code Signing Certificates](https://gregoryszorc.com/docs/apple-codesign/main/apple_codesign_certificate_management.html#apple-codesign-certificate-management) (note: no need to actually use rcodesign, but the information provided was gold)

### Requirements:

- XCode installed (latest non-beta release)
- `cargo install cargo-bundle`
- Signing/Distribution Certificates:
  - _Note: Each developer needs to have the full private+public portion of the certificates. We need to still figure out an easy way of sharing these, or each developer can set up their own for their machine. These are the ones that are needed:_
    - Highly recommend reading through [Managing Code Signing Certificates](https://gregoryszorc.com/docs/apple-codesign/main/apple_codesign_certificate_management.html#apple-codesign-certificate-management) to figure this out.
  - Distributing a Mac App Store build (including TestFlight):
    - "3rd Party Mac Developer Application" certificate
    - "3rd Party Mac Developer Installer" certificate
  - Distributing outside of the App Stores:
    - "Developer ID Application" certificate
    - "Developer ID Installer" certificate
- Apple Root Certificates:
  - Make sure you have these latest root certificates installed from [Apple's certificate authority](https://www.apple.com/certificateauthority/) website:
    - Worldwide Developer Relations
    - Apple WWDR
    - Apple Inc. Root
- AppStoreConnect API Key (for uploading to TestFlight)
  - Create one on AppStoreConnect, and save the .p8 file in a ~/.appstoreconnect/private_keys/

Find the IDs of your certificates with `security find-identity`

- These need to be "Valid". One reason they aren't is if you don't have the latest root certificates installed. DO NOT try to change the trust settings in the keychain app, that will always cause them to be invalid.

```shell
Policy: X.509 Basic
  Matching identities
  1) B4691FECA4D94C522088EF52CDDB5E4D503DF210 "Developer ID Installer: Semen Korzunov (X86RP53R29)"
  2) DDEC1C73C018C49211622D5D36C4B3E50F60E5E0 "Developer ID Application: Semen Korzunov (X86RP53R29)"
  3) AF38E4040F5AE950B7147CDF04CD99EED5A49F06 "3rd Party Mac Developer Application: Semen Korzunov (X86RP53R29)"
  4) 3797AA396E6FFF7224BAC8E560769771A5C15FE0 "3rd Party Mac Developer Installer: Semen Korzunov (X86RP53R29)"
     4 identities found

  Valid identities only
  1) B4691FECA4D94C522088EF52CDDB5E4D503DF210 "Developer ID Installer: Semen Korzunov (X86RP53R29)"
  2) DDEC1C73C018C49211622D5D36C4B3E50F60E5E0 "Developer ID Application: Semen Korzunov (X86RP53R29)"
  3) AF38E4040F5AE950B7147CDF04CD99EED5A49F06 "3rd Party Mac Developer Application: Semen Korzunov (X86RP53R29)"
  4) 3797AA396E6FFF7224BAC8E560769771A5C15FE0 "3rd Party Mac Developer Installer: Semen Korzunov (X86RP53R29)"
     4 valid identities found
```

### Test Flight or App Store release

1. Build the release bundle:

```shell
cargo bundle --release
```

Add additional plist data:

```shell
/usr/libexec/PlistBuddy -c "Add :DTCompiler string com.apple.compilers.llvm.clang.1_0" target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTCompiler string com.apple.compilers.llvm.clang.1_0" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTPlatformBuild string 15F31d" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTPlatformName string macosx" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTPlatformVersion string 14.5" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTSDKBuild string 23F73" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTSDKName string macosx14.5" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTXcode string 1540" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTXcodeBuild string 15F31d" "target/release/bundle/osx/Shelv.app/Contents/Info.plist"
```

2. Embed the provision profile into the bundle.

- Note: The provision profile needs to be associated with the certificate you are using to sign. Create one on AppStoreConnect, and then download the .provisionprofile file.

```shell
cp ~/Downloads/mac_app_store_prov_profile.provisionprofile target/release/bundle/osx/Shelv.app/Contents/embedded.provisionprofile
```

3. Code Sign: Use "3rd Party Mac Developer Application" signing key

- Note: Make sure you only have 1 valid identity for each type. If neccaary, you can pass the actual ID instead to the --sign parameter to disambiguate them.

```shell
codesign --sign "3rd Party Mac Developer Application" --entitlements distribution/entitlements.xml -v target/release/bundle/osx/Shelv.app
```

4. Packaging: Use "3rd Party Mac Developer Installer" signing key

```shell
productbuild --sign "3rd Party Mac Developer Installer" --component target/release/bundle/osx/Shelv.app /Applications target/release/bundle/osx/Shelv.pkg
```

5. Validate the package

- This step is important, as the rest of Apple tool's are useless at telling you what is wrong, and will pretend everything was successful while the build never appears
- In order to use the tool, you need to have an AppStoreConnect key (see [instructions](https://gregoryszorc.com/docs/apple-codesign/main/apple_codesign_getting_started.html#obtaining-an-app-store-connect-api-key))

```shell
xcrun altool --validate-app -f target/release/bundle/osx/Shelv.pkg -t macos --apiKey "HRG65U3FX8" --apiIssuer "804cc69c-4df1-44ae-b829-c8d144aea43d"
```

6. Upload the build

```shell
xcrun altool --upload-app -f target/release/bundle/osx/Shelv.pkg -t macos --apiKey "HRG65U3FX8" --apiIssuer "804cc69c-4df1-44ae-b829-c8d144aea43d"
```

### Outside of App Store distribution

1. Code Sign

```shell
codesign --sign "Developer ID Application" --deep -v -f -o runtime --timestamp target/release/bundle/osx/Shelv.app
```

2. Create a installer package with the "Developer ID Installer: Semen Korzunov" identity

```shell
productbuild --sign "Developer ID Installer" --component target/release/bundle/osx/Shelv.app /Applications target/release/bundle/osx/Shelv.pkg
```

- Note: You can also create a disk image (the .dmg files you normally get). I haven't tried doing this one yet, but there are [Build a Disk Image instructions](https://developer.apple.com/forums/thread/701581#701581021) to do this.

4. Submit the .pkg installer to be notorized

```shell
xcrun notarytool submit --issuer "804cc69c-4df1-44ae-b829-c8d144aea43d" --key-id "HRG65U3FX8" --key ~/.appstoreconnect/private_keys/AuthKey_HRG65U3FX8.p8 target/release/bundle/osx/Shelv.pkg
```

You can check the history with `xcrun notarytool history` access the log of the specific submission with `xcrun notarytool log`:

```shell
xcrun notarytool history --issuer "804cc69c-4df1-44ae-b829-c8d144aea43d" --key-id "HRG65U3FX8" --key ~/.appstoreconnect/private_keys/AuthKey_HRG65U3FX8.p8

xcrun notarytool log --issuer "804cc69c-4df1-44ae-b829-c8d144aea43d" --key-id "HRG65U3FX8" --key ~/.appstoreconnect/private_keys/AuthKey_HRG65U3FX8.p8 "{ID from previous command output}"
```

5. (Optional) Staple the package (this allows for installing offline without the computering screaming "Virus!")
   `xcrun stapler staple -v target/release/bundle/osx/Shelv.pkg`
