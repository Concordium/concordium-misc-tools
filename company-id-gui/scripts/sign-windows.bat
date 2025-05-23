@echo off
setlocal enabledelayedexpansion

REM Check if signing is enabled
if not "%WINDOWS_SIGN%"=="1" (
    echo Skipping signing.
    exit /b 0
)

REM Ensure that WINDOWS_SM_KEYPAIR_ALIAS and WINDOWS_PKCS11_CONFIG are set in the environment
if "%WINDOWS_SM_KEYPAIR_ALIAS%"=="" (
    echo Error: WINDOWS_SM_KEYPAIR_ALIAS environment variable must be set.
    exit /b 1
)

if "%WINDOWS_PKCS11_CONFIG%"=="" (
    echo Error: WINDOWS_PKCS11_CONFIG environment variable must be set.
    exit /b 1
)

if "%~1"=="" (
    echo Error: No input path provided.
    exit /b 1
)

REM Assign environment variables to script variables
set KEYPAIR_ALIAS=%WINDOWS_SM_KEYPAIR_ALIAS%
set CONFIG=%WINDOWS_PKCS11_CONFIG%
set INPUT=%~1

echo Signing environment:
echo - Input file: %INPUT%
echo - File exists:
if exist "%INPUT%" (echo YES) else (echo NO)
echo - Using config file: %CONFIG%
echo - Config file exists:
if exist "%CONFIG%" (echo YES) else (echo NO)

echo Executing: smctl sign --keypair-alias "%KEYPAIR_ALIAS%" --input "%INPUT%" --config-file "%CONFIG%" --verbose --exit-non-zero-on-fail --failfast

smctl sign --keypair-alias "%KEYPAIR_ALIAS%" --input "%INPUT%" --config-file "%CONFIG%" --verbose --exit-non-zero-on-fail --failfast

if %ERRORLEVEL% neq 0 (
    echo Signing failed with error code %ERRORLEVEL%.
    exit /b %ERRORLEVEL%
)

echo Signing completed successfully.
exit /b 0

