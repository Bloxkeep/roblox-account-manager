import assert from 'node:assert/strict';
import { errorEmbed } from '../worker/errors.js';

// The other-device errors must describe the current rejoin controller: wait for
// the remote session to leave, with the global-realm and Auto-rejoin conditions.
for (const code of [273, 276]) {
  for (const language of ['en', 'vi']) {
    const answer = errorEmbed(language, code);
    const behavior = answer.fields[0].value;
    const advice = answer.fields[1].value;
    assert.match(answer.title, new RegExp(String(code)));
    assert.match(behavior, language === 'en' ? /waits while the other device plays/ : /chờ khi thiết bị kia còn chơi/);
    assert.match(behavior, language === 'en' ? /global Roblox account with Auto-rejoin on/ : /Roblox quốc tế đã bật Tự vào lại/);
    assert.match(behavior, language === 'en' ? /no longer in a game/ : /không còn ở trong game/);
    assert.match(advice, /Roblox VNG/);
    console.log(`PASS ${code} ${language}: wait, presence, Auto-rejoin and realm conditions`);
  }
}
