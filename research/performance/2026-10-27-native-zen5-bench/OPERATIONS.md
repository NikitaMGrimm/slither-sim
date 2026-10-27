# Diagnostic collection operations

During the diagnostic schedule, new SSH sessions began returning
`Permission denied (publickey)` at approximately 20:43 UTC. The established
SSH channel continued streaming the sequential bench; its first v0.7.0 repeat
had completed. A direct BatchMode retry reproduced the error. No kernel,
security, service or credential settings were changed to bypass it.

Renewing the existing SSH certificate through the normal signed-in client
restored BatchMode access. This changed no guest security settings or access
scope. The established diagnostic channel and sequential holds continued
without interruption. No completed hold was repeated for this connection issue.
