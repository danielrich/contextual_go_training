// Replay engine reference lines on a copy. This is a viewer, not a legality oracle.
export function replayVariation(problem,moves,limit=moves.length){
 const board=problem.board.map(row=>[...row]),size=problem.size,markers=new Map(),passes=[];
 const adjacent=(x,y)=>[[x-1,y],[x+1,y],[x,y-1],[x,y+1]].filter(([a,b])=>a>=0&&b>=0&&a<size&&b<size);
 function chain(x,y){const color=board[y][x],stones=[],seen=new Set(),queue=[[x,y]];let liberty=false;while(queue.length){const [a,b]=queue.pop(),key=`${a},${b}`;if(seen.has(key))continue;seen.add(key);stones.push([a,b]);for(const [c,d]of adjacent(a,b)){if(!board[d][c])liberty=true;else if(board[d][c]===color&&!seen.has(`${c},${d}`))queue.push([c,d]);}}return{stones,liberty};}
 let color=problem.to_play==='B'?1:2,played=0,error=null;
 for(const move of moves.slice(0,Math.max(0,limit))){const number=played+1;if(String(move).toLowerCase()==='pass'){passes.push({number,color});played++;color=3-color;continue;}
  const match=/^([A-HJ-T])(\d+)$/.exec(String(move));if(!match){error='Invalid coordinate in reference line.';break;}const x='ABCDEFGHJKLMNOPQRST'.indexOf(match[1]),y=size-Number(match[2]);
  if(x<0||x>=size||y<0||y>=size){error='Reference move is outside this board.';break;}
  if(board[y][x]){error='Reference move is on an occupied intersection.';break;}
  board[y][x]=color;
  for(const [a,b]of adjacent(x,y)){if(board[b][a]!==3-color)continue;const group=chain(a,b);if(!group.liberty)for(const [c,d]of group.stones){board[d][c]=0;markers.delete(`${c},${d}`);}}
  const own=chain(x,y);if(!own.liberty){for(const [a,b]of own.stones){board[b][a]=0;markers.delete(`${a},${b}`);}}
  if(board[y][x])markers.set(`${x},${y}`,{x,y,number,color});played++;color=3-color;
 }
 return{board,markers:[...markers.values()],passes,played,error};
}
