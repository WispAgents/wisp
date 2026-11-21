// New task's Judges, at the end of its Done when line (v1 goal.js launcherGoal #judgesBtn): who judges the next
// goal and how they vote, remembered for the goals after it until you change them: the server's `goal` resource
// defaults (defaults.ts, v1 PATCH /config defaults.goalJudges / goalVote), which a goal sent from here is judged by.

import type { JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { useSignal } from '@wisp/ui';
import { goalDefaults, judgesOf, setGoalDefaults } from './defaults.ts';
import { JudgesButton } from './Judges.tsx';
import { liveJudges } from './rules.ts';
import { useCrew, useHarnesses } from './task.ts';
import './i18n/lazy.ts';

export default function HomeJudges(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const crew = useCrew(c);
  const harnesses = useHarnesses(c);
  const d = useSignal(goalDefaults(c));
  // a judge whose harness or crew member is gone doesn't count (v1 liveJudges)
  return <JudgesButton client={c} judges={liveJudges(judgesOf(d(), crew()), crew(), harnesses())} vote={d().vote} onChange={(j, v) => setGoalDefaults(c, j, v)} />;
}
