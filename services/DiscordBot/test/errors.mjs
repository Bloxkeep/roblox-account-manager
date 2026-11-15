import assert from 'node:assert/strict';
import { errorEmbed } from '../worker/errors.js';

// The other-device errors must describe the current rejoin controller: wait for
// the remote session to leave, with the global-realm and Auto-rejoin conditions.
for (const code of [273, 276]) {
  for (const language of ['en', 'vi']) {
    const answer = errorEmbed(language, code);
    const behavior = answer.fields[0].value;
    const advice = answer.fields[1].value;
    assert.match(answer.author.name, new RegExp(String(code)));
    assert.match(behavior, language === 'en' ? /waits while the other device plays/ : /chờ khi thiết bị kia còn chơi/);
    assert.match(behavior, language === 'en' ? /global Roblox account with Auto-rejoin on/ : /Roblox quốc tế đã bật Tự vào lại/);
    assert.match(behavior, language === 'en' ? /no longer in a game/ : /không còn ở trong game/);
    assert.match(advice, /Roblox VNG/);
    console.log(`PASS ${code} ${language}: wait, presence, Auto-rejoin and realm conditions`);
  }
}

// Suggestions while typing: the most asked with nothing typed, codes by their first digits, codes by words.
{
  const { errorChoices, errorMessage } = await import('../worker/errors.js');
  const names = t => errorChoices('en', t).data.choices.map(c => c.name);
  const first = names('');
  assert.ok(first.length > 10 && first.length <= 25 && first[0].startsWith('277 · '), first[0]);
  assert.ok(names('27').every(n => n.startsWith('27')) && names('27').includes('277 · Connection lost'));
  assert.equal(names('277')[0], '277 · Connection lost');
  assert.ok(names('kick').some(n => n.startsWith('267 ')), names('kick').join(' | '));
  assert.ok(names('timeout').some(n => n.startsWith('266 ')), names('timeout').join(' | '));
  assert.ok(errorChoices('vi', 'mat ket noi').data.choices.some(c => c.value === '277'));
  assert.ok(errorChoices('en', 'x'.repeat(200)).data.choices.length === 0);
  for (const c of errorChoices('en', '2').data.choices)
    assert.ok(c.name.length <= 100 && /^\d+$/.test(c.value));
  // A picked suggestion's value and a typed line both answer for their number.
  assert.equal(errorMessage('en', '277').embeds[0].title, 'Connection lost');
  assert.equal(errorMessage('en', '277 · Connection lost').embeds[0].author.name, 'Roblox error 277');
  assert.match(errorMessage('en', '266').embeds[0].footer.text, /Same as 277/);
  assert.equal(errorMessage('en', 'abc').embeds[0].author.name, 'Roblox error ?');
  console.log('PASS suggestions and answers');
}
