// A call to the bot's object, made once more on a fresh stub when the object was starting over. After a deploy the old
// object goes on with the old code until the cron tells it a newer version is out, and it then stops at once
// (worker.js ensure: ctx.abort): a command that came in that moment got no answer at all ("The application did not
// respond", /online at 22:40 on 2026-10-23, three minutes after a deploy). Cloudflare marks such an error retryable;
// the second try reaches the new object.

/** call(stub) on a stub from get(); again on a new one if the first failed because the object was starting over. */
export async function onceMore(get, call) {
  try {
    return await call(get());
  } catch (e) {
    if (!startingOver(e))
      throw e;
    return await call(get());
  }
}

/** Whether an error from a Durable Object call means it was reset or aborted (the call may be made again). */
export function startingOver(e) {
  return e?.retryable === true || /reset|abort|newer version of the bot/i.test(String(e?.message ?? ''));
}
