#!/bin/bash

# Modify plist to match xcode build output

PLIST_FILE="target/release/bundle/osx/Shelv.app/Contents/Info.plist"

if [ ! -f "$PLIST_FILE" ]; then
  echo "plist file not found at '$PLIST_FILE'!"
  echo "Maybe you forgot to `cargo bundle`?"

  exit 1
fi

echo "Modifying plist: $PLIST_FILE"

/usr/libexec/PlistBuddy -c "Add :DTCompiler string com.apple.compilers.llvm.clang.1_0" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTCompiler string com.apple.compilers.llvm.clang.1_0" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTPlatformBuild string 15F31d" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTPlatformName string macosx" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTPlatformVersion string 14.5" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTSDKBuild string 23F73" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTSDKName string macosx14.5" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTXcode string 1540" "$PLIST_FILE"
/usr/libexec/PlistBuddy -c "Add :DTXcodeBuild string 15F31d" "$PLIST_FILE"

PROVISION_PROFILE_SOURCE="distribution/embedded.provisionprofile"
PROVISION_PROFILE_TARGET="target/release/bundle/osx/Shelv.app/Contents/embedded.provisionprofile"

if [ ! -f "$PROVISION_PROFILE_SOURCE" ]; then
  echo "Provisioning file not found. Please add it to the distribution folder"
  exit 1
fi

# Copy over the provisioning profile
echo "Copying provisioning profile from $PROVISION_PROFILE_SOURCE to $PROVISION_PROFILE_TARGET"
cp $PROVISION_PROFILE_SOURCE $PROVISION_PROFILE_TARGET

# Code Sign
echo "Code signing the app"
codesign --sign "3rd Party Mac Developer Application" --entitlements distribution/entitlements.xml -v target/release/bundle/osx/Shelv.app

SIGNING_RESULT=$?
if [ $SIGNING_RESULT -eq 0 ] 
then 
  echo "Successfully code signed" 
else 
  echo "Error code from code signing: $SIGNING_RESULT" >&2
  exit 1
fi

# Create the package installer
echo "Creating package installer"
productbuild --sign "3rd Party Mac Developer Installer" --component target/release/bundle/osx/Shelv.app /Applications target/release/bundle/osx/Shelv.pkg

PACKAGING_RESULT=$?
if [ $PACKAGING_RESULT -eq 0 ] 
then 
  echo "Successfully signed package" 
else 
  echo "Error code from packaging: $PACKAGING_RESULT" >&2
  exit 1
fi

echo "Package created at target/release/bundle/osx/Shelv.pkg"
ls -lh target/release/bundle/osx/