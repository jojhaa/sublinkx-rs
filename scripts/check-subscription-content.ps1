param([string]$BaseUrl = 'http://127.0.0.1:18092', [string]$Username = 'test-admin', [string]$Password = 'test-pass-20260911')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (([uri]$BaseUrl).Host -notin @('127.0.0.1', 'localhost')) { throw 'Only an isolated local test server is allowed' }
$session = [Microsoft.PowerShell.Commands.WebRequestSession]::new()
$login = Invoke-RestMethod "$BaseUrl/api/v1/auth/login" -Method Post -ContentType 'application/json' -Body (@{username=$Username;password=$Password} | ConvertTo-Json) -WebSession $session
$headers = @{ 'X-CSRF-Token' = $login.data.csrf_token }
function RequestJson($method, $path, $body) {
  Invoke-RestMethod "$BaseUrl$path" -Method $method -Headers $headers -WebSession $session -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 10)
}
function ReadExport($url) {
  $content = (Invoke-WebRequest $url).Content
  if ($content -is [byte[]]) { return [Text.Encoding]::UTF8.GetString($content) }
  return [string]$content
}
if ($login.data.user.must_change_credentials) {
  RequestJson 'Post' '/api/v1/auth/change-credentials' @{username='content-tester';current_password='test-pass-20260911';new_password='content-test-changed-20260911';confirm_password='content-test-changed-20260911'} | Out-Null
  $login = Invoke-RestMethod "$BaseUrl/api/v1/auth/login" -Method Post -ContentType 'application/json' -Body '{"username":"content-tester","password":"content-test-changed-20260911"}' -WebSession $session
  $headers['X-CSRF-Token'] = $login.data.csrf_token
}
$node = (RequestJson 'Post' '/api/v1/nodes' @{ name='Test node'; raw_link="ss://YWVzLTEyOC1nY206dGVzdA@$([guid]::NewGuid().ToString('N')).example.com:443#Test" }).data
$payload = @{ name=('Content test '+[guid]::NewGuid().ToString('N')); default_client='mihomo'; node_ids=@($node.id); include_rules=$false }
$subscription = (RequestJson 'Post' '/api/v1/subscriptions' $payload).data
if ($subscription.include_rules -ne $false) { throw 'Create did not save choice' }
$token = $subscription.token
$path = "/api/v1/subscriptions/$($subscription.id)"
$detail = Invoke-RestMethod "$BaseUrl$path" -Headers $headers -WebSession $session
if ($detail.data.include_rules -ne $false) { throw 'Detail lost choice' }
foreach ($target in @('mihomo','clash')) {
  $body = ReadExport "$BaseUrl/s/${token}?target=$target&mode=best_effort"
  if ($body -notmatch '(?m)^proxies:' -or $body -notmatch '(?m)^proxy-groups:' -or $body -match '(?m)^(dns|rule-providers):' -or $body -notmatch 'MATCH,(节点选择|代理选择)') { throw "Invalid nodes-only $target" }
}
$payload.Remove('include_rules')
$legacyUpdate = (RequestJson 'Put' $path $payload).data
if ($legacyUpdate.include_rules -ne $false) { throw 'Legacy update changed choice' }
$payload.include_rules = $true
$full = (RequestJson 'Put' $path $payload).data
if ($full.token -ne $token) { throw 'Token changed' }
$body = ReadExport "$BaseUrl/s/${token}?target=mihomo"
if ($body -notmatch '(?m)^rules:' -or $body -notmatch '(?m)^dns:') { throw 'Full mode or cache transition failed' }
$payload.include_rules = $false
RequestJson 'Put' $path $payload | Out-Null
$body = ReadExport "$BaseUrl/s/${token}?target=mihomo"
if ($body -match '(?m)^(dns|rule-providers):' -or $body -notmatch 'MATCH,(节点选择|代理选择)') { throw 'Old full response remained cached or catch-all missing' }
Write-Output 'PASS: authenticated create/detail/update; old request preservation; stable token; Clash/Mihomo nodes and groups; both cache transitions'
