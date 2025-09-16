#!/usr/bin/env node

/**
 * Surge.sh deployment script for Bolt
 * Deploys static content to surge with bolt.cktech.org domain
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const SURGE_DOMAIN = 'bolt.cktech.org';
const BUILD_DIR = './dist';
const PACKAGE_DIR = './packages';

function log(message) {
    console.log(`[SURGE] ${message}`);
}

function error(message) {
    console.error(`[ERROR] ${message}`);
    process.exit(1);
}

function exec(command, cwd = process.cwd()) {
    try {
        log(`Executing: ${command}`);
        return execSync(command, {
            cwd,
            stdio: 'inherit',
            encoding: 'utf8'
        });
    } catch (err) {
        error(`Command failed: ${command}\n${err.message}`);
    }
}

function checkSurge() {
    try {
        execSync('surge --version', { stdio: 'pipe' });
        log('✓ Surge CLI found');
    } catch (err) {
        log('Installing Surge CLI...');
        exec('npm install -g surge');
    }
}

function createBuildDirectory() {
    log('Creating build directory...');

    if (fs.existsSync(BUILD_DIR)) {
        exec(`rm -rf ${BUILD_DIR}`);
    }

    fs.mkdirSync(BUILD_DIR, { recursive: true });
    fs.mkdirSync(path.join(BUILD_DIR, 'packages'), { recursive: true });
    fs.mkdirSync(path.join(BUILD_DIR, 'packages', 'arch'), { recursive: true });
    fs.mkdirSync(path.join(BUILD_DIR, 'packages', 'debian'), { recursive: true });
}

function generateStaticSite() {
    log('Generating static site content...');

    // Copy install script
    if (fs.existsSync('./install.sh')) {
        fs.copyFileSync('./install.sh', path.join(BUILD_DIR, 'install.sh'));
    }

    // Create index.html
    const indexHtml = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bolt Container Runtime</title>
    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
    <style>
        .gradient-bg { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); }
        .code-block { background: #1a202c; color: #e2e8f0; }
    </style>
</head>
<body class="bg-gray-50">
    <nav class="gradient-bg text-white p-4">
        <div class="container mx-auto flex justify-between items-center">
            <h1 class="text-2xl font-bold">Bolt</h1>
            <a href="https://github.com/CK-Technology/bolt" class="hover:underline">GitHub</a>
        </div>
    </nav>

    <main class="container mx-auto px-4 py-8">
        <div class="text-center mb-12">
            <h1 class="text-5xl font-bold text-gray-800 mb-4">Bolt Container Runtime</h1>
            <p class="text-xl text-gray-600 max-w-3xl mx-auto">
                Next-generation Rust container runtime with gaming optimizations,
                GPU passthrough, and QUIC networking
            </p>
        </div>

        <div class="grid md:grid-cols-2 gap-8 mb-12">
            <div class="bg-white p-6 rounded-lg shadow-lg">
                <h2 class="text-2xl font-bold mb-4 text-gray-800">🚀 Quick Install</h2>
                <div class="code-block p-4 rounded-lg font-mono text-sm">
                    curl -sSL https://bolt.cktech.org/install.sh | sudo bash
                </div>
                <p class="mt-4 text-gray-600">One-line installation with automatic dependency detection</p>
            </div>

            <div class="bg-white p-6 rounded-lg shadow-lg">
                <h2 class="text-2xl font-bold mb-4 text-gray-800">📦 Package Repositories</h2>
                <ul class="space-y-2">
                    <li><a href="/packages/arch/" class="text-blue-600 hover:underline">Arch Linux</a></li>
                    <li><a href="/packages/debian/" class="text-blue-600 hover:underline">Debian/Ubuntu</a></li>
                </ul>
                <p class="mt-4 text-gray-600">Pre-built packages for popular distributions</p>
            </div>
        </div>

        <div class="grid md:grid-cols-3 gap-6 mb-12">
            <div class="bg-white p-6 rounded-lg shadow-lg text-center">
                <div class="text-4xl mb-4">🎮</div>
                <h3 class="text-xl font-bold mb-2">Gaming Optimized</h3>
                <p class="text-gray-600">Built-in GPU passthrough and low-latency gaming support</p>
            </div>

            <div class="bg-white p-6 rounded-lg shadow-lg text-center">
                <div class="text-4xl mb-4">⚡</div>
                <h3 class="text-xl font-bold mb-2">QUIC Networking</h3>
                <p class="text-gray-600">Modern networking protocol for faster container communication</p>
            </div>

            <div class="bg-white p-6 rounded-lg shadow-lg text-center">
                <div class="text-4xl mb-4">🦀</div>
                <h3 class="text-xl font-bold mb-2">Rust Performance</h3>
                <p class="text-gray-600">Memory-safe, zero-cost abstractions with blazing speed</p>
            </div>
        </div>

        <div class="bg-white p-8 rounded-lg shadow-lg">
            <h2 class="text-3xl font-bold mb-6 text-gray-800">Manual Installation</h2>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold mb-2">1. Download the installer</h3>
                    <div class="code-block p-4 rounded-lg font-mono text-sm">
                        wget https://bolt.cktech.org/install.sh<br>
                        chmod +x install.sh
                    </div>
                </div>

                <div>
                    <h3 class="text-lg font-semibold mb-2">2. Run the installer</h3>
                    <div class="code-block p-4 rounded-lg font-mono text-sm">
                        sudo ./install.sh
                    </div>
                </div>

                <div>
                    <h3 class="text-lg font-semibold mb-2">3. Start the service</h3>
                    <div class="code-block p-4 rounded-lg font-mono text-sm">
                        sudo systemctl start bolt<br>
                        bolt --help
                    </div>
                </div>
            </div>
        </div>
    </main>

    <footer class="gradient-bg text-white py-8 mt-12">
        <div class="container mx-auto text-center">
            <p>&copy; 2024 CK Technology. Open source under MIT License.</p>
            <div class="mt-4">
                <a href="https://github.com/CK-Technology/bolt" class="hover:underline mr-4">GitHub</a>
                <a href="/packages/" class="hover:underline">Packages</a>
            </div>
        </div>
    </footer>

    <script>
        // Add copy to clipboard functionality
        document.querySelectorAll('.code-block').forEach(block => {
            block.style.cursor = 'pointer';
            block.title = 'Click to copy';
            block.addEventListener('click', () => {
                navigator.clipboard.writeText(block.textContent);
                const original = block.style.background;
                block.style.background = '#38a169';
                setTimeout(() => block.style.background = original, 200);
            });
        });
    </script>
</body>
</html>`;

    fs.writeFileSync(path.join(BUILD_DIR, 'index.html'), indexHtml);

    // Create package index pages
    const archIndex = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bolt - Arch Linux Packages</title>
    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
</head>
<body class="bg-gray-50">
    <div class="container mx-auto p-8">
        <h1 class="text-3xl font-bold mb-6">Bolt - Arch Linux Packages</h1>
        <div class="bg-white p-6 rounded-lg shadow">
            <h2 class="text-xl font-bold mb-4">Installation</h2>
            <div class="bg-gray-800 text-green-400 p-4 rounded font-mono">
                # Install from AUR<br>
                yay -S bolt-bin<br><br>
                # Or install manually<br>
                curl -sSL https://bolt.cktech.org/install.sh | sudo bash
            </div>
        </div>
        <a href="/" class="inline-block mt-4 text-blue-600 hover:underline">← Back to main page</a>
    </div>
</body>
</html>`;

    const debianIndex = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bolt - Debian/Ubuntu Packages</title>
    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
</head>
<body class="bg-gray-50">
    <div class="container mx-auto p-8">
        <h1 class="text-3xl font-bold mb-6">Bolt - Debian/Ubuntu Packages</h1>
        <div class="bg-white p-6 rounded-lg shadow">
            <h2 class="text-xl font-bold mb-4">Installation</h2>
            <div class="bg-gray-800 text-green-400 p-4 rounded font-mono">
                # Add repository<br>
                curl -fsSL https://bolt.cktech.org/packages/debian/key.gpg | sudo apt-key add -<br>
                echo "deb https://bolt.cktech.org/packages/debian stable main" | sudo tee /etc/apt/sources.list.d/bolt.list<br><br>
                # Install package<br>
                sudo apt update<br>
                sudo apt install bolt
            </div>
        </div>
        <a href="/" class="inline-block mt-4 text-blue-600 hover:underline">← Back to main page</a>
    </div>
</body>
</html>`;

    fs.writeFileSync(path.join(BUILD_DIR, 'packages', 'arch', 'index.html'), archIndex);
    fs.writeFileSync(path.join(BUILD_DIR, 'packages', 'debian', 'index.html'), debianIndex);

    // Create 404 page
    const notFound = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>404 - Page Not Found | Bolt</title>
    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
</head>
<body class="bg-gray-50 flex items-center justify-center min-h-screen">
    <div class="text-center">
        <h1 class="text-6xl font-bold text-gray-800 mb-4">404</h1>
        <p class="text-xl text-gray-600 mb-8">Page not found</p>
        <a href="/" class="bg-blue-600 text-white px-6 py-3 rounded-lg hover:bg-blue-700">
            Go Home
        </a>
    </div>
</body>
</html>`;

    fs.writeFileSync(path.join(BUILD_DIR, '404.html'), notFound);

    log('✓ Static site generated');
}

function deployToSurge() {
    log('Deploying to Surge...');

    // Check if SURGE_TOKEN is set for CI/CD
    if (process.env.SURGE_TOKEN) {
        log('Using SURGE_TOKEN for authentication');
    } else {
        log('Manual login required - surge login will prompt for credentials');
    }

    // Deploy with surge
    exec(`surge ${BUILD_DIR} ${SURGE_DOMAIN}`);

    log(`✓ Deployed to https://${SURGE_DOMAIN}`);
}

function createCNAME() {
    log('Creating CNAME file...');
    fs.writeFileSync(path.join(BUILD_DIR, 'CNAME'), SURGE_DOMAIN);
    log('✓ CNAME file created');
}

function main() {
    log('Starting Surge deployment for Bolt...');

    checkSurge();
    createBuildDirectory();
    generateStaticSite();
    createCNAME();
    deployToSurge();

    log('Deployment complete!');
    log(`Site available at: https://${SURGE_DOMAIN}`);
}

// Handle command line arguments
const args = process.argv.slice(2);
if (args.includes('--help')) {
    console.log(`
Bolt Surge Deployment Script

Usage: node surge-deploy.js [options]

Options:
  --help     Show this help message
  --build    Only build, don't deploy
  --deploy   Only deploy (assumes build exists)

Environment Variables:
  SURGE_TOKEN    Surge.sh authentication token (for CI/CD)

Examples:
  node surge-deploy.js           # Full build and deploy
  node surge-deploy.js --build   # Build only
  node surge-deploy.js --deploy  # Deploy only
`);
    process.exit(0);
}

if (args.includes('--build')) {
    checkSurge();
    createBuildDirectory();
    generateStaticSite();
    createCNAME();
    log('Build complete!');
} else if (args.includes('--deploy')) {
    if (!fs.existsSync(BUILD_DIR)) {
        error('Build directory not found. Run with --build first.');
    }
    deployToSurge();
} else {
    main();
}