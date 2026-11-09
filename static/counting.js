// A personal counting worksheet, never a replacement for engine adjudication.
const key=(x,y)=>`${x},${y}`;
export function ownershipAt(p,x,y){const v=p.analysis?.ownership?.[y*p.size+x];return Number.isFinite(v)&&Math.abs(v)>=.6?(v>0?'B':'W'):null;}
export function revealOwnership(session){return {...session,skipped:[...new Set([...(session.skipped||[]),'groups'])],counting:{...session.counting,enabled:true,engineVisible:true,revealed:true}};}
export function nextCountingStage(session,stage){return stage===1&&session.counting?.revealed?3:stage+1;}
function chain(p,x,y){const color=p.board[y][x],out=[],seen=new Set(),queue=[[x,y]];while(queue.length){const[a,b]=queue.pop(),k=key(a,b);if(seen.has(k))continue;seen.add(k);out.push(k);for(const[c,d]of[[a-1,b],[a+1,b],[a,b-1],[a,b+1]])if(c>=0&&d>=0&&c<p.size&&d<p.size&&p.board[d][c]===color)queue.push([c,d]);}return out;}
export function markPoint(p,state,x,y,mode){
 const next={...state,marks:{...state.marks},dead:[...(state.dead||[])]};
 if(!Number.isInteger(x)||!Number.isInteger(y)||x<0||y<0||x>=p.size||y>=p.size)return state;
 const k=key(x,y),stone=p.board[y][x];if(stone&&mode!=='dead'&&mode!=='erase')return state;if(!stone&&mode==='dead')return state;
 next.undo=[...(state.undo||[]),{marks:{...state.marks},dead:[...(state.dead||[])]}].slice(-400);
 if(stone){const points=chain(p,x,y),remove=mode==='erase'||next.dead.includes(k);next.dead=remove?next.dead.filter(v=>!points.includes(v)):[...new Set([...next.dead,...points])];}
 else if(mode==='erase'||next.marks[k]===mode)delete next.marks[k];else if(mode==='B'||mode==='W')next.marks[k]=mode;
 return next;
}
export function undoCount(state){const undo=[...(state.undo||[])],previous=undo.pop();return previous?{...state,...previous,undo}:state;}
export function countPosition(p,state){
 const territory={black:0,white:0},alive={black:0,white:0},dead={black:0,white:0},mismatches=[],missed=[];let uncertain=0,unmarked=0;
 const removed=new Set(state.dead||[]);
 for(let y=0;y<p.size;y++)for(let x=0;x<p.size;x++){
  const k=key(x,y),stone=p.board[y][x],mark=state.marks?.[k];
  if(stone){if(removed.has(k)){dead[stone===1?'black':'white']++;territory[stone===1?'white':'black']++;}else alive[stone===1?'black':'white']++;continue;}
  if(mark==='B')territory.black++;else if(mark==='W')territory.white++;else unmarked++;
  const owner=ownershipAt(p,x,y);if(!owner)uncertain++;else if(mark&&mark!==owner)mismatches.push({x,y,owner,mark});else if(!mark)missed.push({x,y,owner});
 }
 const method=['japanese','korean'].includes(p.rules)?'territory':'area';
 const known=Number.isInteger(p.captures?.black)&&Number.isInteger(p.captures?.white);
 const prisoners=known?{black:p.captures.black+dead.white,white:p.captures.white+dead.black}:null;
 const black=method==='territory'?(known?territory.black+prisoners.black:null):territory.black+alive.black;
 const white=method==='territory'?(known?territory.white+prisoners.white+p.komi:null):territory.white+alive.white+p.komi;
 return{method,territory,alive,dead,prisoners,black,white,lead:black===null?null:black-white,unmarked,mismatches,missed,uncertain};
}
