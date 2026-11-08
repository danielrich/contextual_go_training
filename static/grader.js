// This version is part of every immutable problem and must match daemon/src/grade.rs.
export const GRADER_VERSION='whole-board-grader-v1';
export const STAGES=['winner','score','groups','candidates'];
const round=v=>Math.sign(v)*Math.round(Math.abs(v)*10)/10;
export const candidateLoss=(baseline,candidate,player)=>Math.max(0,(baseline-candidate)*(player==='B'?1:-1));
export function validCompleteProblem(p){
 if(!p||![9,13,19].includes(p.size)||!p.id||!['B','W'].includes(p.to_play))return false;
 if(!Array.isArray(p.board)||p.board.length!==p.size||p.board.some(r=>!Array.isArray(r)||r.length!==p.size||r.some(v=>![0,1,2].includes(v))))return false;
 const a=p.analysis;if(!a?.complete||a.grader_version!==GRADER_VERSION||!Number.isFinite(a.score)||!a.model||!(a.visits>0))return false;
 if(!Array.isArray(a.ownership)||a.ownership.length!==p.size*p.size||a.ownership.some(v=>!Number.isFinite(v)))return false;
 if(!Array.isArray(p.legal_moves)||!p.legal_moves.length||new Set(p.legal_moves).size!==p.legal_moves.length)return false;
 if(!Array.isArray(p.groups)||p.groups.some(g=>!['B','W'].includes(g.color)||!Array.isArray(g.stones)||!g.stones.length||g.stones.some(s=>!Array.isArray(s)||s.length!==2||s.some(v=>!Number.isInteger(v)||v<0||v>=p.size))))return false;
 return p.legal_moves.every(move=>typeof move==='string'&&a.candidates?.[move]?.move===move&&Number.isFinite(a.candidates[move].score)&&a.candidates[move].visits>0);
}
export function groupStatus(p,g){const values=g.stones.map(([x,y])=>p.analysis.ownership[y*p.size+x]*(g.color==='B'?1:-1));return values.every(v=>v>=.9)?'secure-looking':values.every(v=>v<=-.7)?'at-risk':'uncertain';}
export function grade(p,stage,answer){
 if(p.analysis.grader_version!==GRADER_VERSION)throw new Error('This problem needs an app update before local grading.');
 const index=STAGES.indexOf(stage);if(index<0)throw new Error('Unknown question');
 const f={stage,next_stage:STAGES[index+1]??null,pending:false,grader_version:GRADER_VERSION},score=p.analysis.score;
 if(stage==='winner'){
  if(!['B','W','even'].includes(answer))throw new Error('Choose Black, White or close');
  f.expected=Math.abs(score)<=2?'even':score>0?'B':'W';f.correct=answer===f.expected;f.message='Close means within 2 estimated points. Commit to your count before seeing the score.';
 }else if(stage==='score'){
  if(typeof answer!=='number'||!Number.isFinite(answer)||Math.abs(answer)>p.size*p.size+400)throw new Error('Enter a finite score estimate');
  f.correct=Math.abs(answer-score)<=3;f.score=round(score);f.error=round(Math.abs(answer-score));f.message='KataGo estimates the final lead with strong play, including komi. Within 3 points earns credit; this is not simply settled territory.';
 }else if(stage==='groups'){
  if(!answer||typeof answer!=='object'||Array.isArray(answer))throw new Error('Assess each marked group');
  f.groups=p.groups.map(g=>{const a=answer[g.id];if(!['secure-looking','at-risk','uncertain'].includes(a))throw new Error('Assess each marked group');const expected=groupStatus(p,g);return{id:g.id,anchor:g.anchor,expected,answer:a,correct:a===expected};});
  f.correct=f.groups.every(g=>g.correct);f.message='Ownership agreement only: secure-looking requires at least 90% own-color ownership at every stone; at-risk requires at least 70% opponent ownership at every stone. Otherwise uncertain. These estimates do not prove life, death or seki.';
 }else{
  const required=Math.min(3,p.legal_moves.length);
  if(!Array.isArray(answer)||answer.length!==required||new Set(answer).size!==required||answer.some(m=>!p.legal_moves.includes(m)))throw new Error(`Choose ${required} different legal candidates, including pass if useful`);
  f.candidates=[];f.missing=[];
  for(const move of answer){const evaluation=p.analysis.candidates[move];if(!evaluation){f.missing.push(move);continue;}const loss=candidateLoss(score,evaluation.score,p.to_play);f.candidates.push({move,loss:round(loss),correct:loss<=2,pv:evaluation.pv,visits:evaluation.visits});}
  f.pending=f.missing.length>0;f.correct=f.pending?null:f.candidates.every(c=>c.correct);if(!f.pending)delete f.missing;
  f.suggestions=p.analysis.suggestions;f.message='Each candidate is independently searched from this position. Within 2 estimated points of the baseline earns credit. Search noise can affect close calls; unevaluated moves remain pending.';
 }
 return f;
}
