#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync, spawn } = require('child_process');
const zlib = require('zlib');

const REPO_OWNER = 'Skyline-23';
const REPO_NAME = 'conductor-kit';
const BINARY_NAME = 'conductor';

function getPlatform() {
  const platform = os.platform();
  if (platform === 'darwin') return 'darwin';
  if (platform === 'linux') return 'linux';
  throw new Error(`Unsupported platform: ${platform}`);
}

function getArch() {
  const arch = os.arch();
  if (arch === 'x64') return 'amd64';
  if (arch === 'arm64') return 'arm64';
  throw new Error(`Unsupported architecture: ${arch}`);
}

function httpsGet(url) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { 'User-Agent': 'conductor-kit-npm' } }, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        httpsGet(response.headers.location).then(resolve).catch(reject);
        return;
      }
      if (response.statusCode !== 200) {
        reject(new Error(`HTTP ${response.statusCode}: ${url}`));
        return;
      }
      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });
    request.on('error', reject);
  });
}

async function getLatestVersion() {
  const url = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`;
  const data = await httpsGet(url);
  const release = JSON.parse(data.toString());
  return release.tag_name.replace(/^v/, '');
}

async function downloadAndExtract(version, platform, arch, destDir) {
  const assetName = `${REPO_NAME}_${version}_${platform}_${arch}.tar.gz`;
  const url = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/v${version}/${assetName}`;
  
  console.log(`Downloading ${assetName}...`);
  const tarGzData = await httpsGet(url);
  
  // Create destination directory
  fs.mkdirSync(destDir, { recursive: true });
  
  // Write tar.gz to temp file
  const tempFile = path.join(os.tmpdir(), assetName);
  fs.writeFileSync(tempFile, tarGzData);
  
  // Extract using tar command
  console.log('Extracting...');
  execSync(`tar -xzf "${tempFile}" -C "${destDir}"`, { stdio: 'inherit' });
  
  // Clean up temp file
  fs.unlinkSync(tempFile);
  
  // Ensure binary is executable
  const binaryPath = path.join(destDir, BINARY_NAME);
  if (fs.existsSync(binaryPath)) {
    fs.chmodSync(binaryPath, 0o755);
  }
  
  return binaryPath;
}

async function main() {
  // Skip postinstall in CI or if CONDUCTOR_SKIP_POSTINSTALL is set
  if (process.env.CI || process.env.CONDUCTOR_SKIP_POSTINSTALL) {
    console.log('Skipping postinstall in CI environment');
    return;
  }

  try {
    const platform = getPlatform();
    const arch = getArch();
    
    console.log(`Platform: ${platform}, Architecture: ${arch}`);
    
    // Get latest version
    const version = await getLatestVersion();
    console.log(`Latest version: ${version}`);
    
    // Download to native/ directory in package
    const packageDir = path.join(__dirname, '..');
    const nativeDir = path.join(packageDir, 'native');
    
    const binaryPath = await downloadAndExtract(version, platform, arch, nativeDir);
    
    console.log(`\nConductor ${version} installed successfully!`);
    console.log(`Binary: ${binaryPath}`);
    
    // Run install command to set up skills/commands
    console.log('\nSetting up skills and commands...');
    const installProcess = spawn(binaryPath, ['install', '--mode', 'link', '--repo', nativeDir], {
      stdio: 'inherit',
    });
    
    installProcess.on('exit', (code) => {
      if (code === 0) {
        console.log('\nSetup complete! You can now use conductor.');
        console.log('Run "conductor --help" for usage information.');
      } else {
        console.log('\nNote: Auto-install skipped. Run "conductor install" manually if needed.');
      }
    });
    
  } catch (error) {
    console.error(`\nWarning: Could not download conductor binary: ${error.message}`);
    console.error('You can install it manually via Homebrew:');
    console.error('  brew install Skyline-23/conductor-kit/conductor-kit');
    // Don't fail the npm install
    process.exit(0);
  }
}

main();
