#!/usr/bin/env bash
set -euo pipefail

# Print the usage message
function usage() {
    echo ""
    echo "Builds, signs and notarizes the installer package for the genesis creator tool with a version number (e.g. '0.8.0')."
    echo ""
    echo "Usage: $0 [ --build VERSION ] [ --build-sign VERSION ] [ --sign PKGFILE VERSION ]"
    echo "  --build: Builds the tool and its flat installer package."
    echo "  --build-sign: Builds, signs and notarizes the tool and its flat installer package."
    echo "  --sign: Signs and notarizes the given installer package."
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
    --help)
        usage
        exit 0
        ;;
    --build)
        if [ -n "${BUILD-}" ] || [ -n "${SIGN-}" ]; then
            echo "ERROR: --build flag can not be used together with the other flags."
            usage
            exit 1
        fi
        if [ -z "${2-}" ]; then
            echo "ERROR: --build requires a version number as an argument."
            usage
            exit 1
        fi
        readonly version="$2"
        readonly BUILD=true
        shift
        ;;
    --build-sign)
        if [ -n "${BUILD-}" ] || [ -n "${SIGN-}" ]; then
            echo "ERROR: --build-sign flag can not be used together with the other flags."
            usage
            exit 1
        fi
        if [ -z "${2-}" ]; then
            echo "ERROR: --build-sign requires a version number as an argument."
            usage
            exit 1
        fi
        readonly version="$2"
        readonly BUILD=true
        readonly SIGN=true
        shift
        ;;
    --sign)
        if [ -n "${BUILD-}" ] || [ -n "${SIGN-}" ]; then
            echo "ERROR: --sign flag can not be used together with the other flags."
            usage
            exit 1
        fi
        if [ -z "${2-}" ]; then
            echo "ERROR: --sign requires a package file as an argument."
            usage
            exit 1
        fi
        if [ -z "${3-}" ]; then
            echo "ERROR: --sign requires a version number as an argument."
            usage
            exit 1
        fi
        pkgFile="${2-}"
        readonly version="$3"
        readonly SIGN=true
        shift
        shift
        ;;
    *)
        echo "Unknown option: $1"
        usage
        exit 1
        ;;
    esac
    shift
done

# At least one of 'sign' and 'build' arguments is required
if [ -z "${BUILD-}" ] && [ -z "${SIGN-}" ]; then
    echo "ERROR: You should provide either --build, --build-sign or --sign."
    usage
    exit 1
fi

readonly teamId="K762RM4LQ3"
readonly developerIdApplication="Developer ID Application: Concordium Software Aps ($teamId)"
readonly developerIdInstaller="Developer ID Installer: Concordium Software Aps ($teamId)"

# Get the location of this script.
macPackageDir="$(cd "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
readonly macPackageDir

readonly rootDir="$macPackageDir/../../../"

readonly buildDir="$macPackageDir/build"
readonly payloadDir="$buildDir/payload"
readonly binDir="$payloadDir/usr/local/bin"
readonly libDir="$payloadDir/usr/local/lib"

readonly pkgFile=${pkgFile-"$buildDir/genesis-creator-$version-unsigned.pkg"}
readonly signedPkgFile="${pkgFile%-unsigned*}.pkg"

if [ "$(arch)" == "arm64" ]; then
    readonly arch="aarch64"
else
    readonly arch="x86_64"
fi

# Log info in green color.
logInfo() {
    local GREEN='\033[0;32m'
    local NOCOLOR='\033[0m'
    printf "\n${GREEN}$@${NOCOLOR}\n"
}

function printVersions() {
    logInfo "Printing versions:"
    echo "cargo version: $(cargo --version)"
    logInfo "Done"
}

function cleanBuildDir() {
    if [ -d "$buildDir" ]; then
        logInfo "Cleaning '$buildDir' folder"
        rm -rf "${buildDir:?}"
        logInfo "Done"
    fi
}

function createBuildDir() {
    logInfo "Creating build folder ..."
    mkdir -p "$binDir"
    mkdir -p "$libDir"
    logInfo "Done"
}

# Compile genesis creator.
function compile() {
    cd "$rootDir"
    logInfo "Building Genesis Creator..."
    cargo build --locked --release --package genesis-creator
    logInfo "Done"
}

# Copy the compiled binary to the build folder.
function copyCompiledItemsToBuildDir() {
    logInfo "Copy genesis creator to '$binDir/"
    cp "target/release/genesis-creator" "$binDir"
    logInfo "Done"
}

# Extracts the installer package contents to the 'build' folder.
function expandInstallerPackage() {
    logInfo "Expanding package..."
    pkgutil --expand "$1" "$buildDir"
    cd "$buildDir"
    # Extract the payload content from the package.
    mv Payload Payload.gz
    gunzip Payload
    cpio -iv <Payload # creates a new folder 'usr'
    # Remove the redundant files.
    rm PackageInfo Bom Payload
    # Move the payload content to the 'payload' folder so that
    # it has the same structure as if it was built from scratch.
    mkdir "$payloadDir"
    mv usr "$payloadDir"
    logInfo "Done"
}

# Signs the binaries to be included in the installer with the developer application certificate.
function signBinaries() {
    logInfo "Signing binaries..."

    # Find and sign all binaries and dylibs
    find "$payloadDir" -type f | while read -r file; do
        logInfo "Signing $file..."
        sudo codesign -f --options runtime -s "$developerIdApplication" "$file"

        # Verify the signature immediately after signing
        logInfo "Verifying $file..."
        if ! codesign --verify --verbose=4 "$file"; then
            logError "Signature verification failed for $file"
            exit 1
        fi
    done

    logInfo "All binaries signed and verified ✅"
}

# Signs the installer package with the developer installer certificate.
function signInstallerPackage() {
    logInfo "Signing installer package..."
    productSign --sign "$developerIdInstaller" "$pkgFile" "$signedPkgFile"
    logInfo "Done"
}

# Builds the installer package.
function buildInstallerPackage() {
    logInfo "Building package..."
    pkgbuild --identifier software.concordium.genesis-creator \
        --version "$version" \
        --install-location / \
        --root "$payloadDir" \
        "$pkgFile"
    logInfo "Done"
}

# Notarizes the installer package and wait for it to finish.
# If successful, a notarization 'ticket' will be created on Apple's servers for the product.
# To enable offline installation without warnings, the ticket should be stapled onto the installer.
function notarize() {
    logInfo "Notarizing..."
    xcrun notarytool submit \
        "$signedPkgFile" \
        --apple-id "$APPLEID" \
        --password "$APPLEIDPASS" \
        --team-id "$teamId" \
        --wait
    logInfo "Done"
}

# Staple the notarization ticket onto the installer.
function staple() {
    logInfo "Stapling..."
    xcrun stapler staple "$signedPkgFile"
    logInfo "Done"
}

# Signs, builds and notarizes the installer package.
function signBuildAndNotarizeInstaller() {
    local tmpFile
    tmpFile="/tmp/genesis-creator-$(date +%s).pkg"
    cp "$pkgFile" "$tmpFile"
    cleanBuildDir
    expandInstallerPackage "$tmpFile"
    rm "$tmpFile"
    signBinaries
    buildInstallerPackage
    signInstallerPackage
    notarize
    staple
    logInfo "Signing complete"
    logInfo "Signed installer located at:\n$signedPkgFile"
}

# Builds the tool and creates the installer package.
function buildInstaller() {
    cleanBuildDir
    createBuildDir
    compile
    copyCompiledItemsToBuildDir
    buildInstallerPackage
    logInfo "Build complete"
    logInfo "Installer located at:\n$pkgFile"
}

function main() {
    if [ -n "${BUILD-}" ]; then
        printVersions
        buildInstaller
    fi

    if [ -n "${SIGN-}" ]; then
        signBuildAndNotarizeInstaller
    fi
}

main