#!/usr/bin/env python3
"""Build editable SVG review frames and a local multiwindow design browser.
Only writes inside design/ui-ux-refactor-v2; never imports or changes app code.
"""
from pathlib import Path
from html import escape
import json
import re

ROOT = Path(__file__).resolve().parents[1]
C = dict(bg="#0c1015", surface="#10171e", raised="#18232e", border="#334553",
         text="#e6edf5", muted="#a9b7c6", mint="#67d1b9", amber="#f2b977",
         red="#f3a6a0", blue="#8ec9f2")
FRAMES = []

def screen(id, title, group, kind="form", **kw):
    item = dict(id=id, title=title, group=group, kind=kind, **kw)
    FRAMES.append(item)
    return item

screen("01-welcome", "Start your workspace", "01 Workspace", "welcome",
       actions=[("Add connection", "08-connection-form"), ("Import SSH aliases", "09-ssh-options")],
       states=["No hosts", "No sources", "No automatic connection"])
screen("02-logs-live", "All logs", "01 Workspace", "logs",
       states=["Following live", "3 sources selected"], native=["openView", "closeView", "viewNotice", "viewPage", "cancelPage"])
screen("03-logs-paused", "Errors · paused", "01 Workspace", "logs", paused=True,
       states=["Paused viewport", "Background ingest continues", "New arrivals across all sources"])
screen("04-saved-views", "Saved views", "02 Query and views", "table",
       columns=["View name", "Query", "Source scope", "Action"],
       rows=[["Errors only", "level:error", "api.log, worker", "Open"],
             ["Slow requests", "duration>200ms", "api.log", "Open"],
             ["Checkout incident", "trace_id:req-42", "Unavailable source ID", "Review"]],
       note="Restoring a view never starts sources or grants access. Missing source IDs stay visible.",
       actions=[("Save current view", "04b-save-view"), ("Query syntax", "05-query-help"), ("Restore unavailable scope", "05b-unavailable-scope")],
       native=["savedViews", "saveView", "deleteView"])
screen("04b-save-view", "Save current view", "02 Query and views",
       fields=[("View name", "Checkout timeouts"), ("Query", "trace_id:req-42 AND level:error"),
               ("Source IDs", "api.log · worker")],
       note="Only query and source IDs are saved. The next session may have different source IDs.",
       actions=[("Save view", "04-saved-views"), ("Cancel", "02-logs-live")])
screen("05-query-help", "Query syntax and validation", "02 Query and views",
       fields=[("Search query", "trace_id:req-42 AND duration>200ms")],
       cards=[("Supported", "Literal text · AND · field:value · quoted strings · duration comparisons in ms/s"),
              ("Exact identifiers", "trace_id, order_id, user_id match exactly. message matches literal substrings."),
              ("Unsupported", "OR / NOT / regex are not supported. Show a useful parse error near the query.")],
       note="Invalid query example: level:error OR timeout → OR is unsupported; return to the last valid filter.",
       actions=[("See invalid query", "05c-query-error"), ("Open Saved views", "04-saved-views")])
screen("05b-unavailable-scope", "Saved view has unavailable sources", "02 Query and views", "logs", unavailable=True,
       states=["Unavailable source IDs", "Restore never starts source"])
screen("05c-query-error", "Invalid query", "02 Query and views", "logs", invalid=True,
       states=["Query parse error", "Last valid results retained", "Edit query to recover"])
screen("06-columns", "Columns and source scope", "02 Query and views",
       checks=[("Time", True), ("Host / environment", True), ("Source", True), ("Level", True),
               ("Message · required", True), ("trace_id", False), ("duration_ms", False)],
       fields=[("Source scope", "All retained sources / explicit checked source IDs")],
       note="Columns are bounded. Message stays visible. Source colors also have readable labels.",
       actions=[("Apply to log tab", "02-logs-live"), ("Source health", "45-source-health")])
screen("07-connections", "Connections", "03 Connections", "table",
       columns=["Host", "Environment", "State", "Actions"],
       rows=[["api-prod-01", "Production", "Connected", "Select · Disconnect"],
             ["staging-01", "Staging", "Reconnecting", "Details"],
             ["app-server", "Development", "Saved profile", "Connect"]],
       note="Selecting a host changes context only. Disconnect explicitly stops its retry. Production is a label.",
       actions=[("Add connection", "08-connection-form"), ("Import aliases", "09-ssh-options"), ("Connection details", "10-connection-error")],
       native=["status", "savedHosts", "connect", "disconnect", "setEnvironment"])
screen("08-connection-form", "Add connection", "03 Connections",
       fields=[("SSH alias or hostname *", "app-server"), ("User · optional", "From SSH configuration"),
               ("Port · optional", "From SSH configuration"), ("Environment", "Development ▾")],
       note="Uses your existing SSH agent. Host keys must already be trusted by OpenSSH.",
       actions=[("Connect host", "07-connections"), ("Advanced SSH", "09-ssh-options"), ("Cancel", "07-connections")])
screen("09-ssh-options", "SSH options and static aliases", "03 Connections",
       fields=[("IdentityAgent socket · optional", "Use configured SSH agent"), ("Static SSH aliases", "app-server · staging-01")],
       checks=[("Allow native dynamic SSH configuration for this session · may run local commands", False)],
       cards=[("Static import", "Read static aliases without running ssh -G or Match exec."),
              ("Host key policy", "Changed or unknown host keys are not automatically accepted.")],
       actions=[("Import static aliases", "08-connection-form"), ("Back to connection", "08-connection-form")],
       native=["importConfig"])
screen("10-connection-error", "Connection needs attention", "03 Connections",
       banner=("Connection blocked · host key is unknown or changed", "red"),
       fields=[("Host", "staging-01"), ("Connection state", "Needs action"), ("Retry", "Stopped until the issue is resolved")],
       cards=[("Next step", "Verify and trust the host using your normal OpenSSH workflow, then reconnect."),
              ("Network reconnect", "For a temporary disconnect, show bounded retry state and a visible Disconnect action.")],
       actions=[("Back to connections", "07-connections"), ("Source health", "45-source-health")])
screen("11-sources", "Reading sources", "04 Sources", "table",
       columns=["Source", "Host / type", "State", "Actions"],
       rows=[["/var/log/app/api.log", "api-prod-01 · File", "Reading", "Pin · Stop"],
             ["worker", "api-prod-01 · Docker", "Reading", "Stop"],
             ["nginx.service", "api-prod-01 · Journal", "Reconnecting", "Details · Stop"]],
       note="3 / 8 sources. Stop is explicit. Pinned paths never auto-stream. Tags are hints only.",
       actions=[("Add file", "12-file-source"), ("Discover", "13-discovery"), ("Docker", "16-docker-source"), ("Journal", "17-journal-source")],
       native=["stop", "pin", "pins"])
screen("12-file-source", "Open a log file", "04 Sources",
       fields=[("Host", "api-prod-01 · Production"), ("Absolute log path *", "/var/log/app/api.log"),
               ("Path suggestions", "/var/log/app/api.log · ready"), ("Pinned paths", "/var/log/nginx/error.log")],
       cards=[("Reading continuity", "Rotation / truncation and ambiguous continuity are shown as quality markers, never hidden."),
              ("Explicit start", "Selecting a path only fills this form. Start file begins reading.")],
       actions=[("Start file", "11-sources"), ("Discover metadata", "13-discovery"), ("Cancel", "11-sources")],
       native=["complete", "start", "preview"])
screen("13-discovery", "Discover sources", "04 Sources", "table",
       columns=["Candidate", "Provider / access", "Activity / provenance", "Action"],
       rows=[["/var/log/nginx/error.log", "var_log · Ready", "Growing · shallow metadata", "Start"],
             ["worker", "Docker · Ready", "Container metadata", "Start"],
             ["nginx.service", "systemd unit · Ready", "Selected unit metadata", "Start"],
             ["/srv/app/api.log", "Project root · Denied", "Permission denied", "Unavailable"]],
       note="Discovery reads bounded metadata only. No candidate is started automatically. Tags can be edited as hints.",
       actions=[("Discovery scope", "14-discovery-scope"), ("Partial / denied", "15-discovery-partial"), ("Start selected source", "11-sources")],
       native=["discover", "cancelDiscovery"])
screen("14-discovery-scope", "Discovery scope and consent", "04 Sources",
       fields=[("Project root", "/srv/my-app"), ("PM2 logs directory", "/home/app/.pm2/logs"),
               ("Selected PIDs · comma separated", "412, 518"), ("Selected systemd units", "nginx.service"),
               ("Approved config root", "/etc/nginx"), ("Exact config files · one per line", "/etc/nginx/nginx.conf")],
       checks=[("Docker / Compose metadata", True), ("/var/log · depth 2", True),
               ("Allow reading these exact config files for path hints", False)],
       note="Config fields are disabled until config consent is checked. Existing topology or MCP consent does not apply.",
       actions=[("Discover metadata", "13-discovery"), ("Cancel discovery", "11-sources")])
screen("15-discovery-partial", "Discovery returned partial metadata", "04 Sources", "table",
       banner=("Partial results · one directory denied; discovery reached its work cap", "amber"),
       columns=["Candidate", "Access", "Reason", "Action"],
       rows=[["/var/log/app/api.log", "Ready", "Metadata available", "Start"],
             ["/root/private.log", "Denied", "Insufficient permissions", "Disabled"],
             ["More project paths", "Unknown", "Bounded scan incomplete", "Narrow scope"]],
       note="Show ready results plus precise reasons. Cancel stops discovery requests; it does not stop existing readers.",
       actions=[("Narrow scope", "14-discovery-scope"), ("Open file directly", "12-file-source")])
screen("16-docker-source", "Read Docker logs", "04 Sources",
       fields=[("Host", "api-prod-01 · Production"), ("Container", "worker · synthetic-container"),
               ("Aliases / tags · hints only", "worker, background-jobs")],
       cards=[("Streams", "Docker stdout/stderr remain distinct. Restart/replay continuity may be partial."),
              ("Permission", "Docker CLI and container log access are required on the host.")],
       actions=[("Start container", "11-sources"), ("Discover containers", "13-discovery")])
screen("17-journal-source", "Read a systemd journal", "04 Sources",
       fields=[("Host", "api-prod-01 · Production"), ("Discovered unit", "nginx.service")],
       cards=[("Requirements", "Python 3, journalctl and permission to read this unit are required."),
              ("Cursor continuity", "Journal cursor is held in RAM. Expired cursors and incomplete catch-up are visible.")],
       actions=[("Start journal", "11-sources"), ("Discover units", "13-discovery")])
screen("18-inspector-fields", "Event details · req-42", "05 Inspector", "inspector",
       native=["detail", "cancelDetail"],
       states=["Pinned event", "Detail bounded to 1 MiB / 4096 nodes", "Lazy JSON children"])
screen("19-inspector-raw", "Raw event", "05 Inspector", "inspector", variant="raw")
screen("20-inspector-stack", "Stack group", "05 Inspector", "inspector", variant="stack")
screen("21-inspector-expired", "Event no longer retained", "05 Inspector",
       banner=("This event expired from retention", "amber"),
       cards=[("What happened", "Closing a tool or retaining its reference does not pin event data forever."),
              ("Recover", "Return to retained logs or run the original query again. Some stack parts can also expire.")],
       actions=[("Return to retained logs", "03-logs-paused"), ("Requery", "02-logs-live")])
screen("22-trace-input", "Trace a request", "06 Investigation",
       fields=[("Source scope · pinned", "api.log · worker · nginx/error.log"), ("ID type", "trace_id ▾ / order_id"),
               ("Exact ID *", "req-42"), ("From local observed ms · optional", "1795163058000"),
               ("To local observed ms · optional", "1795163118000")],
       note="Local observed-time window. Cross-host clock offsets and missing parents are unknown.",
       actions=[("Trace request", "23-trace-result"), ("Use current log scope", "22-trace-input")],
       native=["debugRead"])
screen("23-trace-result", "Trace · req-42", "06 Investigation", "trace",
       actions=[("Open evidence", "29-evidence"), ("Inspect event", "18-inspector-fields")])
screen("24-topology-config", "Topology configuration", "06 Investigation",
       fields=[("Selected host", "api-prod-01 · Production"), ("Exact Compose files · one per line", "/srv/app/compose.yaml"),
               ("Approved roots · one per line", "/srv/app"), ("Allowed literal endpoint hostnames", "db, cache")],
       checks=[("Allow bounded reads and metadata freshness checks for these exact files", False)],
       note="Static JSON / block-YAML subset only. No .env, runtime env, execution or automatic interpolation.",
       actions=[("Read topology · after consent", "25-topology-graph"), ("Service mappings", "41-mappings")],
       native=["topology", "topologyRevision"])
screen("25-topology-graph", "Configured topology", "06 Investigation", "topology",
       states=["Configured / inferred provenance", "Retained error counts", "Rate unknown", "15-second metadata freshness"])
screen("26-topology-stale", "Topology needs a fresh read", "06 Investigation",
       banner=("Approved configuration changed or freshness is unavailable", "amber"),
       cards=[("Graph invalidated", "The old snapshot is not shown as current. Retained counts do not imply a live error rate."),
              ("Refresh", "Reread only the explicitly approved files and roots. Consent remains scoped.")],
       actions=[("Review and reread configuration", "24-topology-config"), ("View source health", "45-source-health")])
screen("27-error-clusters", "Error clusters", "06 Investigation", "table",
       columns=["Sanitized signature", "Retained count", "Sources", "Evidence"],
       rows=[["Connection timeout", "18", "api.log", "Open sample"],
             ["Upstream reset", "6", "nginx/error.log", "Open sample"],
             ["Queue retry exceeded", "3", "worker", "Open sample"]],
       note="Sample counts, bounded retained evidence. This is a proposed desktop surface for the existing get_error_clusters MCP capability.",
       actions=[("Inspect sample", "29-evidence"), ("Filter logs", "03-logs-paused")])
screen("28-service-finder", "Locate a service", "06 Investigation", "table",
       columns=["Candidate", "Host / source", "Why matched", "Action"],
       rows=[["checkout-api", "api-prod-01 · api.log", "Declared service alias", "Open logs"],
             ["checkout-worker", "api-prod-01 · worker", "Project / service alias", "Review"]],
       note="Two candidates are ambiguous; choose explicitly. Locate does not Start sources or widen grants. Proposed desktop surface for locate_service.",
       actions=[("Open selected logs", "02-logs-live"), ("Manage aliases", "41-mappings")])
screen("29-evidence", "Evidence and preceding context", "06 Investigation", "table",
       columns=["Ref", "Source", "Quality", "Redacted evidence"],
       rows=[["ev-sample-01", "api.log", "Retained", "Connection timeout · req-42"],
             ["ev-sample-02", "api.log", "Preceding context", "Started request · req-42"],
             ["ev-sample-03", "worker", "Partial", "Job queued · req-42"]],
       note="Opaque references and context cursors expire. Redacted evidence only; coverage and gaps remain visible.",
       actions=[("Inspector", "18-inspector-fields"), ("Export evidence", "30-export-scope"), ("Trace", "23-trace-result")])
screen("30-export-scope", "Export incident · scope", "07 Export",
       fields=[("Scope", "3 selected sources / selected event IDs"), ("Query", "trace_id:req-42"),
               ("Format", "NDJSON ▾ / spreadsheet-safe CSV"), ("Observed from · Unix ms", "Optional"),
               ("Observed to · Unix ms", "Optional")],
       note="Redaction and coverage metadata are applied before preparing the exact output.",
       actions=[("Prepare exact preview", "31-export-preview"), ("Cancel", "02-logs-live")],
       native=["prepareExport"])
screen("31-export-preview", "Export incident · exact preview", "07 Export", "preview",
       preview='{"metadata":{"coverage":"partial","gaps":["worker replay"]}}\n{"source":"api.log","level":"error","message":"Connection timeout","token":"[REDACTED]"}',
       fields=[("Absolute destination", "/home/user/incident.ndjson")],
       checks=[("Replace an existing file at this path", False)],
       note="Prepared bytes expire after 60 seconds. Output cap: 2 MiB in the current alpha. Save only this exact preview.",
       actions=[("Save exact bytes", "31b-export-saved"), ("Cancel", "30-export-scope"), ("Expired / write error", "32-export-expired")],
       native=["commitExport", "cancelPrepared"])
screen("31b-export-saved", "Incident saved", "07 Export",
       banner=("Sanitized incident saved", "mint"),
       fields=[("Destination", "/home/user/incident.ndjson"), ("Format", "NDJSON"), ("Coverage", "Partial, included in metadata")],
       actions=[("Back to logs", "02-logs-live"), ("New export", "30-export-scope")])
screen("32-export-expired", "Prepare a fresh export", "07 Export",
       banner=("Preview expired, scope changed, or write failed", "amber"),
       cards=[("No stale save", "Expired or invalidated bytes cannot be committed. Destination and filesystem errors remain explicit."),
              ("Recovery", "Prepare a new preview. Overwrite stays off until explicitly chosen again.")],
       actions=[("Prepare again", "30-export-scope"), ("Cancel", "02-logs-live")])
screen("33-ai-disabled", "Explain Error settings", "08 AI and MCP",
       fields=[("Provider", "Disabled ▾")],
       cards=[("Optional feature", "Explain Error stays off until a provider is configured. MCP works without a model."),
              ("Privacy", "Each explanation needs a separate exact preview and consent. No automatic cloud fallback.")],
       actions=[("Configure Ollama", "34-ollama"), ("Configure BYOK", "35-byok"), ("MCP access", "38-mcp-grants")],
       native=["explainProvider", "configureExplainer"])
screen("34-ollama", "Local Ollama provider", "08 AI and MCP",
       fields=[("Provider", "Local Ollama"), ("Exact endpoint", "http://127.0.0.1:11434/api/chat"),
               ("Installed model", "local-model")],
       note="Literal loopback only. Use a model already installed locally; TailPro does not install it or proxy the request.",
       actions=[("Apply provider", "33-ai-disabled"), ("Preview explanation", "36-explain-preview")])
screen("35-byok", "BYOK provider and credential", "08 AI and MCP",
       fields=[("Provider", "HTTPS OpenAI-compatible API"), ("Exact endpoint", "https://provider.example/v1/chat/completions"),
               ("Model", "configured-model"), ("API key · saved using OS Secret Service", "••••••••")],
       banner=("Credential-store state must be checked before saving", "amber"),
       note="If OS Secret Service is unavailable, BYOK is unavailable. No plaintext persistence, redirect, proxy or fallback.",
       actions=[("Apply provider", "33-ai-disabled"), ("Store unavailable state", "35b-secret-unavailable")],
       native=["storeProviderKey"])
screen("35b-secret-unavailable", "BYOK unavailable on this system", "08 AI and MCP",
       banner=("OS Secret Service is unavailable", "red"),
       cards=[("Credential was not stored", "Do not offer an insecure plaintext fallback."),
              ("Available choices", "Configure local Ollama if already available, or resolve the operating-system secret-store setup.")],
       actions=[("Local Ollama", "34-ollama"), ("Provider settings", "33-ai-disabled")])
screen("36-explain-preview", "Explain Error · review payload", "08 AI and MCP", "preview",
       preview='Evidence ev-sample-01\n{"source":"api.log","message":"Connection timeout","trace_id":"req-42","secret":"[REDACTED]"}',
       fields=[("Provider / model", "Local Ollama · local-model"), ("Exact endpoint", "http://127.0.0.1:11434/api/chat")],
       checks=[("Send these exact redacted bytes to this provider and model", False)],
       note="Send is disabled until consent is checked. Scope/provider changes or TTL expiry require a fresh preview.",
       actions=[("Send after consent", "37-explain-result"), ("Cancel", "18-inspector-fields")],
       native=["prepareExplanation", "explain"])
screen("37-explain-result", "Explanation and hypotheses", "08 AI and MCP",
       banner=("Model output · untrusted hypotheses", "amber"),
       cards=[("Selected evidence", "ev-sample-01 · api.log · Connection timeout"),
              ("Hypothesis", "The upstream may not have responded within the configured timeout. Verify with retained trace evidence."),
              ("Suggested checks", "Inspect relevant service logs and confirm the deployed revision in your coding client. TailPro executes no suggested action.")],
       actions=[("Open evidence", "29-evidence"), ("Trace request", "23-trace-result"), ("New preview", "36-explain-preview")])
screen("38-mcp-grants", "Coding client access", "08 AI and MCP", "table",
       columns=["Client", "Allowed scope", "Expires", "Action"],
       rows=[["Sample coding client", "api.log, worker", "In 42 minutes · sample", "Revoke"],
             ["No global grant", "Other sources unavailable", "Session only", "—"]],
       note="Grants expire after one hour, app restart or source configuration changes. No global client config is edited.",
       actions=[("Grant source access", "39-mcp-enroll"), ("Revoke / expired", "40-mcp-revoked"), ("Observation workflow", "46-observation")],
       native=["grants", "enroll", "revoke"])
screen("39-mcp-enroll", "Grant selected sources", "08 AI and MCP",
       fields=[("Client name *", "Sample coding client"), ("MCP executable path", "/absolute/path/to/tailpro-mcp")],
       checks=[("api-prod-01 · Production · api.log", True), ("api-prod-01 · Production · worker", False),
               ("I allow this coding client and its model provider to receive redacted logs from these sources", False)],
       note="Grant is disabled until source scope, client and separate consent are provided.",
       actions=[("Grant after consent", "39b-enrollment"), ("Cancel", "38-mcp-grants")])
screen("39b-enrollment", "One-time enrollment configuration", "08 AI and MCP", "preview",
       preview='{"mcpServers":{"tailpro":{"command":"/absolute/path/to/tailpro-mcp",\n"env":{"TAILPRO_TOKEN":"[ONE-TIME SECRET — SAMPLE ONLY]"}}}}',
       note="Illustrative config only. Real configuration includes runtime/session/grant fields. Secret shown once; hide on close. Never store in layout or screenshots.",
       actions=[("Copy sample configuration", "38-mcp-grants"), ("Hide secret", "38-mcp-grants")])
screen("40-mcp-revoked", "Access revoked or expired", "08 AI and MCP",
       banner=("The coding client no longer has access", "amber"),
       cards=[("Why access ends", "Manual revoke, one-hour expiry, app restart, source configuration or relevant mapping change."),
              ("Re-enrollment", "Select the intended source scope and approve a new grant. Old tokens and cursors cannot revive access.")],
       actions=[("Review grants", "38-mcp-grants"), ("Grant again", "39-mcp-enroll")])
screen("41-mappings", "Service and code mappings", "09 Service mappings", "table",
       columns=["Source", "Service / project", "Checkout declaration", "Action"],
       rows=[["api.log", "checkout-api / shop", "shop-api · /home/user/shop-api", "Edit · Remove"],
             ["worker", "checkout-worker / shop", "shop-worker · revision unknown", "Edit · Remove"]],
       note="Declarations only: no repository opening/indexing. Coding client verifies checkout and deployed revision.",
       actions=[("Add mapping", "42-mapping-form"), ("Find service", "28-service-finder")],
       native=["serviceMappings", "saveServiceMapping", "deleteServiceMapping"])
screen("42-mapping-form", "Edit service mapping", "09 Service mappings",
       fields=[("Source *", "api.log ▾"), ("Service *", "checkout-api"), ("Project", "shop"),
               ("Aliases · comma separated", "api, checkout"), ("Repository label *", "shop-api"),
               ("Declared local absolute root *", "/home/user/shop-api"), ("Declared revision · unverified", "Optional")],
       note="Saving this mapping revokes existing related agent grants and contexts. Relative stack hints still require client verification.",
       actions=[("Save mapping", "41-mappings"), ("Cancel", "41-mappings")])
screen("43-command-palette", "Switch host, source or path", "10 Help and health",
       fields=[("Search · Ctrl K", "/var/log/app/")],
       cards=[("Host · api-prod-01", "Choose host context only; never connect implicitly."),
              ("Reading source · api.log", "Choose source scope."),
              ("/var/log/app/api.log · ready", "Use path in source form or Start selected source explicitly.")],
       actions=[("Use path in source form", "12-file-source"), ("Start selected source", "11-sources"), ("Keyboard help", "44-shortcuts")])
screen("44-shortcuts", "Keyboard shortcuts", "10 Help and health", "table",
       columns=["Shortcut", "Action", "Focus rule"],
       rows=[["Ctrl K", "Command palette", "Current window"],
             ["Ctrl F", "Focus query", "Current log tab"],
             ["Space", "Pause / resume viewport", "Log grid focus only"],
             ["Arrow keys / Enter", "Select / inspect event", "Log grid focus only"],
             ["Escape", "Close topmost popup / float", "Never closes OS window or resumes live"],
             ["Tab / Shift Tab", "Move keyboard focus", "Modal traps focus; close restores opener"]],
       note="Float / Detach / Dock are explicit tab-menu actions. No new global shortcut is reserved without review.")
screen("45-source-health", "Source health and viewer diagnostics", "10 Help and health", "table",
       columns=["Source", "State", "Continuity", "Action"],
       rows=[["api.log · Production", "Reading", "Rotation marker retained", "Inspect"],
             ["worker · Production", "Reconnecting", "Catch-up incomplete / gap", "Details"],
             ["nginx.service", "Reading", "Journal cursor recovered", "Inspect"]],
       note="Health stays outside query filters. Diagnostics: retained rows, charged RAM, discarded/evicted counts; frame/core/IPC/page/cache measurements are live values, not performance certification.",
       actions=[("Connections", "07-connections"), ("Source manager", "11-sources"), ("Return to logs", "02-logs-live")])
screen("46-observation", "MCP observation workflow", "08 AI and MCP",
       cards=[("1 · Capture baseline", "Coding client calls get_recent_logs(limit=0) with the intended source scope and query."),
              ("2 · Reproduce externally", "Edit, build and reproduce in your coding client. TailPro does not execute those actions."),
              ("3 · Read bounded evidence", "search_logs uses the observation cursor and unchanged scope/query; continue next_cursor to the fence."),
              ("4 · Inspect and qualify", "Use get_log_context / inspect_context. Gaps or a missing expected request are insufficient evidence of a fix.")],
       note="Tools: list_sources · get_recent_logs · search_logs · get_log_context · locate_service · get_error_clusters · trace_id · inspect_context",
       actions=[("Grants", "38-mcp-grants"), ("Evidence", "29-evidence")])
screen("47-window-manager", "Windows and tabs", "11 Window behavior", "table",
       columns=["Surface", "Placement", "Context", "Action"],
       rows=[["All logs / Errors", "Main document tabs", "Independent query / source scope", "Float · Detach"],
             ["Trace · req-42", "Separate window", "Pinned 3-source scope", "Focus · Dock back"],
             ["Event details", "Floating tool", "Pinned event reference", "Focus · Detach · Dock"],
             ["Connections / Sources", "Utility window", "Shared connection registry", "Focus"],
             ["Settings / MCP", "Utility window", "Session settings and grants", "Focus"]],
       note="Proposed cap: 4 OS windows total / 8 document tabs, subject to resource validation. Reuse singleton utility windows. Close main → Dock tools or Exit all; never silently abandon readers.",
       actions=[("Main logs", "02-logs-live"), ("Trace tool", "23-trace-result"), ("Inspector tool", "18-inspector-fields")])

class SVG:
    def __init__(self,w,h): self.w=w;self.h=h;self.parts=[]
    def rect(self,x,y,w,h,fill=None,stroke=None,r=0):
        self.parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill or C["surface"]}"'+(f' stroke="{stroke}"' if stroke else '')+'/>')
    def text(self,x,y,text,size=14,color=None,weight=400,mono=False):
        family="DejaVu Sans Mono, monospace" if mono else "Inter, DejaVu Sans, sans-serif"
        self.parts.append(f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{color or C["text"]}">{escape(str(text))}</text>')
    def group(self,id): self.parts.append(f'<g id="{id}">')
    def end(self): self.parts.append('</g>')
    def lines(self,x,y,text,max_chars=98,size=14,color=None,line=24):
        import textwrap
        lines=[]
        for part in text.split("\n"): lines.extend(textwrap.wrap(part,width=max_chars) or [""])
        for i,s in enumerate(lines): self.text(x,y+i*line,s,size,color)
        return len(lines)*line
    def button(self,x,y,text,target=None,primary=False,width=None):
        width=width or min(500,max(100,len(text)*7.1+28))
        self.group("Action-"+re.sub(r"[^a-zA-Z0-9-]","-",text))
        self.rect(x,y,width,36,C["mint"] if primary else C["raised"],C["mint"] if primary else C["border"],6)
        self.text(x+14,y+23,text,13,C["bg"] if primary else C["text"],600 if primary else 400)
        if target:
            self.parts.append(f'<a href="../index.html?screen={target}" target="_top"><title>{escape(text)}</title><rect x="{x}" y="{y}" width="{width}" height="36" fill="#ffffff" fill-opacity="0"/></a>')
        self.end()
        return width
    def field(self,x,y,label,value,width=680):
        self.text(x,y,label,12,C["muted"],500)
        self.rect(x,y+11,width,38,C["bg"],C["border"],6)
        self.text(x+12,y+36,value,13,C["text"])
    def note(self,x,y,text,width):
        height=35+self.lines(x+14,y+24,text,max_chars=max(30,int((width-28)/7.4)),size=12,color=C["muted"],line=19)
        return height
    def save(self,path,title):
        body=''.join(self.parts)
        path.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" viewBox="0 0 {self.w} {self.h}"><title>{escape(title)}</title><desc>Editable TailPro design proposal v2. Synthetic data. Not the production application.</desc>{body}</svg>')

def shell(f):
    sizes={"logs":(1440,900),"welcome":(1440,900),"trace":(1200,820),"topology":(1280,840),"inspector":(880,800),"table":(1180,820)}
    w,h=sizes.get(f["kind"],(1040,820))
    s=SVG(w,h)
    s.rect(0,0,w,h,C["bg"])
    s.group("Native-window-titlebar")
    s.rect(0,0,w,36,C["surface"])
    s.text(16,24,"TailPro — "+f["title"],12,C["muted"])
    s.text(w-98,24,"−      □      ×",14,C["muted"]);s.end()
    s.group("Tool-header")
    s.rect(0,36,w,58,C["surface"])
    s.text(24,72,"TailPro",20,C["mint"],650)
    s.text(125,71,"Local workspace",12,C["muted"])
    s.button(w-320,47,"Dock back","47-window-manager",width=120)
    s.button(w-186,47,"Windows","47-window-manager",width=115)
    s.end()
    s.group("Status-bar")
    s.rect(0,h-30,w,30,C["surface"])
    s.text(20,h-10,"DESIGN v2 · Sample data · Read-only SSH",11,C["muted"])
    s.text(w-295,h-10,"Tab menu: Float / Detach / Dock",11,C["muted"]);s.end()
    return s,w,h

def actions(s,f,x,y,maxw):
    for i,(label,target) in enumerate(f.get("actions",[])):
        width=min(maxw,max(100,len(label)*7.1+28))
        if x+width>s.w-25: x=28;y+=48
        s.button(x,y,label,target,primary=i==0,width=width);x+=width+12

def table(s,f,w,h):
    x=28;y=160
    cols=f["columns"];width=w-56
    s.group("Data-table")
    s.rect(x,y,width,40,C["raised"],C["border"],6)
    cw=width/len(cols)
    for i,t in enumerate(cols):s.text(x+14+cw*i,y+26,t,12,C["muted"],600)
    y+=40
    for ri,row in enumerate(f["rows"]):
        s.rect(x,y,width,58,C["surface"] if ri%2==0 else C["bg"])
        for ci,t in enumerate(row):s.lines(x+14+cw*ci,y+25,str(t),max_chars=max(14,int((cw-22)/7)),size=12,line=18)
        y+=58
    s.end()
    if f.get("banner"):
        s.rect(28,y+22,w-56,52,C["raised"],C[f["banner"][1]],6)
        s.text(42,y+53,f["banner"][0],13,C[f["banner"][1]])
        y+=80
    s.note(28,y+24,f.get("note",""),w-56)
    actions(s,f,28,h-94,w-56)

def logs(s,f,w,h):
    s.group("Global-navigation")
    s.button(w-690,47,"Connections","07-connections",width=140)
    s.button(w-538,47,"Sources","11-sources",width=100)
    s.button(w-426,47,"Tools","47-window-manager",width=90)
    s.end()
    s.group("Collapsible-source-sidebar")
    s.rect(0,94,224,h-124,C["surface"])
    s.text(16,127,"Hosts & sources",13,C["text"],600);s.text(196,127,"«",16,C["muted"])
    s.button(12,144,"Add connection","08-connection-form",width=198)
    for i,(text,col) in enumerate([("● api-prod-01   PROD","amber"),("  ☑ api.log","mint"),("  ☑ worker","mint"),("  ☑ nginx/error.log","amber"),("○ staging-01","muted")]):
        s.text(16,214+i*37,text,13,C[col])
    s.text(16,436,"Saved views",12,C["muted"],600)
    s.button(12,451,"Errors only","03-logs-paused",width=198)
    s.button(12,499,"Manage views","04-saved-views",width=198)
    s.button(12,h-92,"Source health","45-source-health",width=198)
    s.end()
    x=244
    s.group("Detachable-document-tabs")
    s.rect(x,105,180,38,C["raised"],C["mint"],6);s.text(x+15,130,"All logs       ×",13)
    s.rect(x+192,105,175,38,C["surface"],C["border"],6);s.text(x+207,130,"Errors         ×",13,C["muted"])
    s.text(x+390,130,"+",22,C["mint"])
    s.button(w-219,106,"Tab menu  ▾","47-window-manager",width=190);s.end()
    s.text(x,183,f["title"],22,C["text"],600)
    s.button(w-289,156,"Add source","11-sources",width=126);s.button(w-151,156,"Export","30-export-scope",width=120)
    query="level:error OR timeout" if f.get("invalid") else "level:error" if f.get("paused") else "Search logs or enter a query…"
    s.field(x,206,"Search retained logs · Ctrl F",query,w-x-30)
    py=280
    if f.get("invalid") or f.get("unavailable"):
        msg="OR is unsupported. Edit the query; last valid results remain below." if f.get("invalid") else "Saved view contains unavailable source IDs. Restore did not start sources."
        s.rect(x,270,w-x-30,42,C["raised"],C["amber"],6);s.text(x+12,296,msg,12,C["amber"]);py=333
    s.text(x,py+12,"PAUSED" if f.get("paused") else "LIVE",12,C["amber"] if f.get("paused") else C["mint"],600)
    s.button(x+85,py-11,"Resume live" if f.get("paused") else "Pause","02-logs-live" if f.get("paused") else "03-logs-paused",width=126)
    s.text(x+230,py+12,"3 selected sources · pinned tab scope",12,C["muted"])
    s.button(w-279,py-11,"Saved views","04-saved-views",width=130)
    s.button(w-139,py-11,"Columns","06-columns",width=110)
    ty=py+43;tw=w-x-30
    s.group("Virtualized-log-table")
    s.rect(x,ty,tw,34,C["raised"])
    colx=[x+12,x+142,x+291,x+453]
    for xx,txt in zip(colx,["Time / local observed","Host / Source","Level","Message"]):s.text(xx,ty+23,txt,11,C["muted"],600)
    msgs=["GET /api/users/42 200 36ms","worker[3] completed job id=98124","Upstream response slow · 582ms","POST /v1/orders 201 124ms","Started job id=98125","Connection timeout · trace_id=req-42","GET /health 200 3ms","Retry scheduled · job=98125","Client closed connection early","GET /metrics 200 4ms"]
    for i,msg in enumerate(msgs):
        yy=ty+34+i*38
        selected=i==5 and f.get("paused")
        s.rect(x,yy,tw,38,"#203831" if selected else C["surface"] if i%2==0 else C["bg"])
        s.text(colx[0],yy+24,f"16:24:{18+i:02d}.042",11,mono=True)
        s.text(colx[1],yy+24,"api-prod / "+("worker" if i%3==1 else "api.log"),11,C["mint"],mono=True)
        level="ERROR" if i==5 else "WARN" if i in (2,8) else "INFO"
        s.text(colx[2],yy+24,level,11,C["red"] if level=="ERROR" else C["amber"] if level=="WARN" else C["blue"],mono=True)
        s.text(colx[3],yy+24,msg,12,mono=True)
        if i==5:s.parts.append(f'<a href="../index.html?screen=18-inspector-fields" target="_top"><title>Inspect event</title><rect x="{x}" y="{yy}" width="{tw}" height="38" fill="#fff" fill-opacity="0"/></a>')
    s.end()
    bottom=h-122
    s.text(x,bottom,"Viewport paused · 42 new arrivals across sources" if f.get("paused") else "3 sources streaming · health visible outside query",12,C["muted"])
    s.button(x,bottom+20,"Load older page","03-logs-paused",width=160)
    s.button(x+172,bottom+20,"Trace ↗","23-trace-result",width=120)
    s.button(x+304,bottom+20,"Event details ↗","18-inspector-fields",width=160)
    s.text(w-335,bottom+44,"Health / gaps / retention →",12,C["muted"])

def inspector(s,f,w,h):
    s.text(28,138,f["title"],23,weight=600)
    s.text(28,174,"ERROR · api-prod-01 / api.log · PRODUCTION",13,C["red"])
    s.text(28,200,"Pinned event ev-sample-01 · local observed time · req-42",12,C["muted"])
    for i,(label,target) in enumerate([("Fields","18-inspector-fields"),("Raw","19-inspector-raw"),("Stack","20-inspector-stack")]):
        s.button(28+i*130,224,label,target,width=118)
    variant=f.get("variant","fields")
    s.group("Event-content-"+variant)
    if variant=="fields":
        fields=[("message","Connection timeout"),("trace_id","req-42"),("duration_ms","1200"),("level","error"),("context","▸ Object · expand lazy children"),("quality","Retained · source timestamp basis visible")]
        for i,(key,value) in enumerate(fields):
            y=288+i*49;s.rect(28,y,w-56,43,C["surface"],r=4)
            s.text(42,y+27,key,13,C["muted"],mono=True);s.text(245,y+27,value,12,mono=True)
            s.text(w-70,y+27,"⧉",17,C["mint"])
    elif variant=="raw":
        text='{"level":"error","message":"Connection timeout",\n "trace_id":"req-42","duration_ms":1200,\n "context":{"service":"checkout-api"}}\n\nRaw text is escaped and selectable.\nCopy remains a deliberate local action.\nNo HTML / ANSI / URL execution.'
        s.rect(28,286,w-56,260,C["surface"],C["border"],6)
        s.lines(44,316,text,90,13,line=30)
    else:
        for i,txt in enumerate(["Part 1 · Connection timeout","Part 2 · at checkout.request (src/api.ts:42)","Part 3 · preceding stack part expired"]):
            s.rect(28,290+i*72,w-56,58,C["surface"],C["border"],6);s.text(42,325+i*72,txt,13)
        s.note(28,528,"Relative code hints are declarations only. The coding client verifies checkout and revision.",w-56)
    s.end()
    s.button(28,h-115,"Export event","30-export-scope",width=160)
    s.button(200,h-115,"Explain error","36-explain-preview",width=160)
    s.button(372,h-115,"Evidence context","29-evidence",width=176)
    s.text(28,h-55,"Closing releases detail budget. It does not resume the parent log viewport.",12,C["muted"])

def draw(f):
    s,w,h=shell(f)
    s.group("Screen-content-"+f["id"])
    if f["kind"]=="logs": logs(s,f,w,h)
    elif f["kind"]=="inspector": inspector(s,f,w,h)
    elif f["kind"]=="welcome":
        s.text(80,230,"A focused workspace for your logs",34,weight=650)
        s.lines(80,282,"Connect a host, choose a source and start reading.\nYour data stays local; no remote agent is installed.",size=17,line=32,color=C["muted"])
        actions(s,f,80,400,w-160)
        for i,(title,body) in enumerate([("1 · Connect","Use an SSH alias or host."),("2 · Choose sources","Files, Docker or Journal."),("3 · Investigate","Open tools as tabs or windows.")]):
            s.rect(80+i*430,540,400,160,C["surface"],C["border"],8)
            s.text(104+i*430,578,title,17,C["mint"],600);s.lines(104+i*430,616,body,38,size=14)
    else:
        s.text(28,138,f["title"],24,weight=600)
        if f["kind"]=="table": table(s,f,w,h)
        elif f["kind"]=="trace":
            s.text(28,177,"trace_id:req-42 · pinned scope: api.log / worker / nginx",13,C["muted"])
            s.rect(28,198,w-56,48,C["surface"],C["amber"],6)
            s.text(42,228,"Partial retained evidence · clock offsets / causal links unknown",13,C["amber"])
            for i,(label,duration) in enumerate([("api.log",1200),("worker",80),("nginx/error.log",None)]):
                y=294+i*120;s.text(28,y,label,14,weight=600)
                if duration:
                    width=max(90,duration/1200*(w-270));s.rect(220,y-22,width,36,"#245a4c",r=5)
                    s.text(233,y+2,f"{duration} ms",13,C["mint"])
                else:s.text(220,y,"◆ Event only · no duration_ms",13,C["mint"])
            s.note(28,660,"Bars compare explicit durations. Horizontal position does not encode causal start time. No retained match is not proof of absence.",w-56)
            actions(s,f,28,h-94,w-56)
        elif f["kind"]=="topology":
            s.text(28,178,"api-prod-01 · exact approved Compose files · configured / inferred",13,C["muted"])
            nodes=[(60,260,"web","Configured · compose.yaml", "2 retained errors · rate unknown"),(470,260,"api","Configured · compose.yaml","18 retained errors · rate unknown"),(880,260,"db","Inferred · literal hostname","Count unknown · no mapped source")]
            s.parts.append('<path d="M350 334 H470 M760 334 H880" fill="none" stroke="#a9b7c6" stroke-width="2" stroke-dasharray="7 5"/>')
            for x,y,title,provenance,count in nodes:
                s.rect(x,y,290,160,C["surface"],C["border"],8)
                s.text(x+18,y+36,title,20,C["mint"],600)
                s.text(x+18,y+76,provenance,12,C["muted"])
                s.lines(x+18,y+106,count,31,size=12)
            s.note(28,485,"Counts are retained snapshots, not live rates. Metadata checked every 15 seconds; changed/unavailable config invalidates the graph.",w-56)
            s.button(28,600,"Open mapped logs","02-logs-live",width=180)
            s.button(220,600,"Configuration","24-topology-config",width=160)
            s.button(392,600,"Stale state","26-topology-stale",width=140)
        else:
            y=170
            if f.get("banner"):
                text,col=f["banner"];s.rect(28,y,w-56,48,C["raised"],C[col],6)
                s.text(42,y+30,text,13,C[col]);y+=72
            if f["kind"]=="preview":
                s.rect(28,y,w-56,158,C["surface"],C["border"],6)
                s.lines(44,y+30,f["preview"],max_chars=112,size=12,line=23);y+=182
            fields=f.get("fields",[])
            compact=len(fields)>5
            for i,(label,value) in enumerate(fields):
                s.field(28,y,label,value,w-56);y+=67 if compact else 78
            for label,checked in f.get("checks",[]):
                s.rect(28,y+2,16,16,C["mint"] if checked else C["bg"],C["border"],3)
                if checked:s.text(30,y+15,"✓",12,C["bg"],600)
                s.lines(56,y+15,label,max_chars=115,size=12,line=18);y+=38
            for title,body in f.get("cards",[]):
                s.rect(28,y,w-56,84,C["surface"],C["border"],6)
                s.text(44,y+28,title,14,C["mint"],600);s.lines(44,y+54,body,max_chars=112,size=12,color=C["muted"],line=19);y+=96
            if f.get("note"):s.note(28,min(y+8,h-178),f["note"],w-56)
            actions(s,f,28,h-94,w-56)
    s.end()
    f["width"],f["height"]=w,h
    f["file"]="02-frames/"+f["id"]+".svg"
    s.save(ROOT/f["file"],f["title"])

def main():
    for folder in ["00-brief","01-foundations","02-frames","03-flows","04-handoff","05-concepts"]:
        (ROOT/folder).mkdir(parents=True,exist_ok=True)
    for f in FRAMES:draw(f)
    (ROOT/"00-brief/screens.json").write_text(json.dumps(FRAMES,ensure_ascii=False,indent=2)+"\n")
    # The review UI consumes embedded JSON so file:// works without fetch/CORS.
    data=json.dumps(FRAMES,ensure_ascii=False).replace("</","<\\/")
    (ROOT/"screens-data.js").write_text("window.TAILPRO_SCREENS = "+data+";\n")
    print(f"Built {len(FRAMES)} editable SVG frames in {ROOT/'02-frames'}")
if __name__=="__main__":main()
