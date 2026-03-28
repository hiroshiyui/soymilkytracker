---
name: security-audit
description: Perform project-wide security audits.
---

When performing a security audit, always follow these steps:

1. **Audit Dependencies** — check for known vulnerabilities in all dependencies (client and server). Use the appropriate tool for the stack (e.g. `cargo audit`, `npm audit`).

2. **Static Analysis** — review code for common web security issues: XSS, CSRF, insecure file upload handling, injection vulnerabilities (SQL, command), and improper authentication/authorization in API endpoints.

3. **WASM & Browser Security** — check that the WASM module does not expose unsafe operations to the host, and that Content Security Policy headers are correctly configured.

4. **File Upload Pipeline** — validate that uploaded sound font files are type-checked, size-limited, stored outside the web root, and never executed on the server.

5. **Report Findings** — document all identified risks, classify them by severity (Critical, High, Medium, Low), and provide specific remediation steps for each.
