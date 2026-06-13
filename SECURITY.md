# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

1. **DO NOT** create a public GitHub issue for security vulnerabilities
2. Email security concerns to: ckelley@ghostkellz.sh
3. Or use GitHub's private vulnerability reporting feature

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fixes (optional)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Fix Timeline**: Depends on severity
  - Critical: 24-72 hours
  - High: 1-2 weeks
  - Medium: 2-4 weeks
  - Low: Next release cycle

## Security Considerations

### Container Runtime Security

Bolt is a container runtime with elevated privileges. Key security considerations:

- **GPU Passthrough**: Direct GPU device access bypasses container isolation
- **Privilege Escalation**: Running containers may require elevated permissions
- **Device Access**: `/dev/nvidia*` devices provide direct hardware access

### Best Practices

1. **Least Privilege**: Only pass required GPU devices to containers
2. **Image Trust**: Only run containers from trusted sources
3. **Network Isolation**: Use appropriate network policies
4. **Update Regularly**: Keep Bolt and drivers updated

### Known Limitations

- GPU passthrough inherently reduces container isolation
- Native GPU support requires kernel module access
- Some features require root/sudo access

## Dependency Security

We use `cargo audit` to check for known vulnerabilities in dependencies.

Run security audit:
```bash
cargo audit
```

### Known Dependency Issues (Updated 2026-04-04)

Some transitive dependencies have known vulnerabilities that cannot be immediately resolved due to version constraints. We've updated what we can:

**Resolved:**
- `quinn-proto` updated to 0.11.14 (RUSTSEC-2026-0037 fixed)
- `time` updated to 0.3.47 (RUSTSEC-2026-0009 fixed)

**Remaining (transitive, awaiting upstream):**

| Crate | Issue | Status |
|-------|-------|--------|
| `aws-lc-sys` | RUSTSEC-2026-0045/46/47/48 | Transitive via rustls; awaiting compatible aws-lc-rs update |
| `rustls-webpki` | RUSTSEC-2026-0049 | Transitive via rustls/quinn |
| `bincode` | Unmaintained (warning) | Considering migration to `postcard` or `rkyv` |
| `rustls-pemfile` | Unmaintained (warning) | Transitive via reqwest 0.11 |
| `lru` | Unsound IterMut (warning) | Transitive via aws-sdk-s3 |

**Mitigation**: These are primarily DoS vulnerabilities or warnings about maintenance status. They affect features not actively used in Bolt's core GPU functionality. We monitor for upstream fixes and will update when compatible versions are available.

## Disclosure Policy

- We will acknowledge receipt within 48 hours
- We will provide regular updates on fix progress
- We will credit reporters (unless anonymity is requested)
- We follow coordinated disclosure practices
