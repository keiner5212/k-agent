# System reminder

The reminder is not a second system prompt. It is the same system string, pasted again onto a user turn so a long chat does not forget it.

The colored version of the example below is [reminder.html](./reminder.html).

## When it is attached

`applySystemReminder` in `src/lib/session-turns.ts` runs on the turns that are about to be sent.

1. Take the system string already built for this send. That is language (if force-response-language is on), global and workspace `AGENTS.md`, then `composeAgentSystem` (`<agent-flow>`, skills not yet loaded, `<clarify>`, `<todos>`, `<tools>`, `<agents>`, `<personality>`, `<rendering>`, `<visual-check>`), then MCP tools and workspace notes when those exist. Personality is already inside that string. A skill that was loaded stays in the chat as its tool result. Its own text says when to call `skill` again.
2. Read `reminderInterval` from settings. The default is 8. The select allows 1 through 500. `0` or an empty system string skips the reminder.
3. Count user turns whose role is `user` and that are not a tool result.
4. When that count is greater than 0 and `count % interval == 0`, append the reminder to the last such user turn.
5. Assistant turns and tool-result turns do not move the counter and do not receive the reminder.

The appended text is:

```text
<system-reminder>
{the full system string}
</system-reminder>
```

If the user turn already has text, the reminder is added after a blank line. If the turn is empty, the reminder is the whole content.

## Example

Interval `4`. The fourth real user turn receives the reminder. The body is the same system string used for that send. The block below is cut so it fits. A real reminder includes every section, including `<personality>`.

```text
Please check the sidebar filter.

<system-reminder>
<language>
LANGUAGE RULE - VERY IMPORTANT
You must reply ONLY in English.
</language>

<personality>
You are the build agent. Be concise.
</personality>

...the rest of the same system string...
</system-reminder>
```

The first three user turns stay unchanged. The next reminder lands on user turn 8, then 12, and so on.
