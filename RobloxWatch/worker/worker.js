// Asks Roblox every minute whether a new Windows build went live, is offered on its beta channel or was uploaded
// ahead of release, and starts the "Roblox watch" workflow (RobloxWatch/watch.py) at once instead of waiting for its
// 10-minute schedule. All checks, Discord posts and the status file are the workflow's; this only notices and wakes it.
const REPO = 'minhhoang0122/bloxkeep';
const WORKFLOW = 'roblox-watch.yml';
const UA = 'Bloxkeep-RobloxWatch (+https://github.com/minhhoang0122/bloxkeep)';
// A failed run isn't started again before this, so a broken download doesn't start one every minute.
const RETRY_AFTER_FAILURE_MS = 15 * 60 * 1000;

export default {
  async scheduled(controller, env, ctx) {
    ctx.waitUntil(check(env));
  },
};

async function check(env) {
  const github = {
    Authorization: `Bearer ${env.WATCH_TOKEN}`,
    'User-Agent': UA,
    'X-GitHub-Api-Version': '2022-11-28',
  };
  const live = await getJson('https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer', { 'User-Agent': UA });
  const beta = await getJson('https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer/channel/zbeta',
    { 'User-Agent': UA }).catch(() => null);
  const tail = await getText('https://setup.rbxcdn.com/DeployHistory.txt', { 'User-Agent': UA, Range: 'bytes=-4000' });
  const uploads = [...tail.matchAll(/New WindowsPlayer \S+ at .+?, file version: (\d+), (\d+), (\d+), (\d+)/g)];
  const upload = uploads.length ? uploads[uploads.length - 1].slice(1).join('.') : null;

  const status = await getJson(`https://api.github.com/repos/${REPO}/contents/multi-status.json?ref=roblox-status`,
    { ...github, Accept: 'application/vnd.github.raw+json' });
  const newLive = status.live?.hash !== live.clientVersionUpload;
  const newBeta = beta?.clientVersionUpload && newer(beta.version, live.version) && status.beta?.hash !== beta.clientVersionUpload;
  const newUpload = upload !== null && newer(upload, live.version) && status.future?.version !== upload;
  if (!newLive && !newBeta && !newUpload)
    return;

  // One run at a time: a run already queued or going will be followed by this check again next minute.
  const runs = await getJson(`https://api.github.com/repos/${REPO}/actions/workflows/${WORKFLOW}/runs?per_page=1`,
    { ...github, Accept: 'application/vnd.github+json' });
  const last = runs.workflow_runs?.[0];
  if (last && last.status !== 'completed')
    return;
  if (last && last.conclusion === 'failure' && Date.now() - Date.parse(last.created_at) < RETRY_AFTER_FAILURE_MS)
    return;

  const reason = newLive ? `live ${live.version} ${live.clientVersionUpload}`
    : newBeta ? `beta ${beta.version} ${beta.clientVersionUpload}` : `uploaded ${upload}`;
  const r = await fetch(`https://api.github.com/repos/${REPO}/actions/workflows/${WORKFLOW}/dispatches`, {
    method: 'POST',
    headers: { ...github, Accept: 'application/vnd.github+json', 'Content-Type': 'application/json' },
    body: JSON.stringify({ ref: 'main', inputs: { reason } }),
  });
  if (!r.ok)
    throw new Error(`starting the workflow: ${r.status} ${await r.text()}`);
  console.log(`started the workflow: ${reason}`);
}

function newer(a, b) {
  const x = a.split('.').map(Number), y = b.split('.').map(Number);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0))
      return (x[i] ?? 0) > (y[i] ?? 0);
  }
  return false;
}

async function getText(url, headers) {
  const r = await fetch(url, { headers });
  if (!r.ok)
    throw new Error(`${url}: ${r.status}`);
  return r.text();
}

async function getJson(url, headers) {
  return JSON.parse(await getText(url, headers));
}
