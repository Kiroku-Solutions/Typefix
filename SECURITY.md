# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

The Kiroku Solutions team takes security seriously. We appreciate your efforts to responsibly disclose vulnerabilities.

**Please do NOT open public GitHub issues for security vulnerabilities.**

Report privately via:

- **Email**: contact@kirokusolutions.com
- **GitHub Security Advisories**: Use the "Report a vulnerability" button on the Security tab of the repository

Include in your report:

1. Description of the vulnerability and impact
2. Steps to reproduce or proof-of-concept
3. Affected versions
4. Your name/handle for credit (optional)

We will:

- Acknowledge receipt within 48 hours
- Provide an initial assessment within 7 days
- Coordinate disclosure timeline with you
- Credit you in the fix release notes (unless you prefer anonymity)

## Security Considerations for Users

TypeFix is designed with security in mind:

- **No network I/O**: The engine never makes outbound connections
- **No file system writes**: The default configuration does not write to disk
- **Memory-safe**: Safe Rust except for OS FFI (windows, macOS, memory profiling) — audited and minimized
- **Fail-silent**: Errors degrade gracefully, never crash the host process
- **No telemetry**: Zero data collection, no analytics, no tracking
- **Deterministic**: Same input always produces same output
- **No persistence**: User dictionaries are opt-in (off by default)

### Sensitive Use Cases

For high-sensitivity environments (PHI, PII, classified data), see:

- [docs/integration-ehr-legal.md](./docs/integration-ehr-legal.md) - HIPAA, EHR, and legal document handling
- [docs/risk-register.md](./docs/risk-register.md) - Risk register with mitigations

### Keyboard Hook Permissions

On Windows, the engine uses low-level keyboard hooks (`WH_KEYBOARD_LL`), which require no special elevation but log a benign security event when installed. On Linux, it uses X11 or evdev. On macOS, it requires Accessibility permission in System Preferences.

## Threat Model

### Assets Protected
- **Keystroke Integrity**: Ensure keystrokes are processed securely within memory.
- **Host System Stability**: A crash in the engine must not lock the host's keyboard or crash the OS.

### Potential Threats & Mitigations
- **Denial of Service (DoS)** via massive inputs or payloads:
  - *Mitigation*: 10MB hard limits on all JSON parsing (WASM included).
  - *Mitigation*: Ring buffers for keystrokes capped at 64 chars to prevent memory exhaustion.
- **Malicious Window Injection** (injecting keystrokes to the wrong window):
  - *Mitigation*: Double-verification of active window ID before and after processing using atomic operations.
- **Memory Exploits**:
  - *Mitigation*: No dynamic allocations in the hot path. Safe Rust guarantees prevent use-after-free and data races.
- **Data Exfiltration**:
  - *Mitigation*: Zero network I/O. The engine has no HTTP or telemetry capabilities.

### Out of Scope
- Protection against kernel-level keyloggers or rootkits.
- Maliciously crafted dictionaries loaded locally by the user (user is responsible for local file integrity).

## Security Updates

Security patches are released as soon as possible after confirmation. Critical fixes may result in a patch release outside the normal release cadence.

Subscribe to repository releases to be notified of security updates.
