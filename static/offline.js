import {nextCountingStage} from './counting.js';
import {validCompleteProblem,STAGES,meanRank} from './grader.js';
const EMPTY=()=>({total:0,skills:Object.fromEntries(STAGES.map(s=>[s,{total:0,correct:0}])),breakdown:[],recent:[]});
const request=r=>new Promise((resolve,reject)=>{r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});
export function createStore(name='whole-board-v2'){
 let opened=null,inflight=null;
 function open(){if(!opened)opened=new Promise((resolve,reject)=>{const r=indexedDB.open(name,1);r.onupgradeneeded=()=>{for(const store of ['problems','packs','sessions','outbox','attempts','meta'])if(!r.result.objectStoreNames.contains(store))r.result.createObjectStore(store,{keyPath:'id'});};r.onsuccess=()=>{r.result.onversionchange=()=>{r.result.close();opened=null;};resolve(r.result);};r.onerror=()=>{opened=null;reject(r.error);};r.onblocked=()=>reject(new Error('Close another old Whole Board tab to update offline storage.'));});return opened;}
 async function transaction(names,mode,fn){const db=await open(),tx=db.transaction(names,mode);const done=new Promise((resolve,reject)=>{tx.oncomplete=()=>resolve();tx.onabort=()=>reject(tx.error||new Error('Offline write was interrupted'));tx.onerror=()=>{};});done.catch(()=>{});try{const value=await fn(tx);await done;return value;}catch(e){try{tx.abort();}catch{}await done.catch(()=>{});throw e;}}
 async function setServerProgress(value){await transaction(['meta'],'readwrite',async tx=>{const s=tx.objectStore('meta'),old=await request(s.get('server-progress'));if(!old||value.total>=old.value.total)await request(s.put({id:'server-progress',value}));});}
 const get=(name,id)=>transaction([name],'readonly',t=>request(t.objectStore(name).get(id)));
 const all=name=>transaction([name],'readonly',t=>request(t.objectStore(name).getAll()));
 const put=(name,value)=>transaction([name],'readwrite',t=>request(t.objectStore(name).put(value)));
 async function meta(id){return (await get('meta',id))?.value;}
 async function setMeta(id,value){await put('meta',{id,value});}
 async function putProblem(p){await put('problems',{...p,cached_at:Date.now()});}
 async function readyProblems(){return (await all('problems')).filter(validCompleteProblem);}
 async function savePack(pack){
  if(!pack?.job?.id||!Array.isArray(pack.problems)||pack.problems.some(p=>!validCompleteProblem(p)))throw new Error('Pack is incomplete; it has not been marked available offline.');
  await transaction(['problems','packs'],'readwrite',async tx=>{for(const p of pack.problems)await request(tx.objectStore('problems').put({...p,cached_at:Date.now()}));await request(tx.objectStore('packs').put({id:pack.job.id,job:pack.job,problem_ids:pack.problems.map(p=>p.id),cached_at:Date.now()}));});
 }
 async function saveSession(s){await transaction(['sessions','meta'],'readwrite',async tx=>{await request(tx.objectStore('sessions').put({...s,updated:Date.now()}));await request(tx.objectStore('meta').put({id:'active-session',value:s.id}));});}
 async function skipQuestion(session){
  if(session.finished||session.feedback||!Number.isInteger(session.stage)||session.stage<0||session.stage>=STAGES.length||session.results?.[STAGES[session.stage]])throw new Error('Only unanswered questions can be skipped.');
  const skipped=[...new Set([...(session.skipped||[]),STAGES[session.stage]])];
  const next={...session,skipped,stage:Math.min(nextCountingStage(session,session.stage),STAGES.length-1),feedback:null,finished:session.stage===STAGES.length-1};
  await saveSession(next);return next;
 }
 async function activeSession(){const id=await meta('active-session');if(!id)return null;const s=await get('sessions',id);if(!s)return null;for(const result of Object.values(s.results||{})){const confirmed=await get('attempts',result.attempt_id);if(confirmed)result.feedback=confirmed.feedback;}const current=s.results?.[STAGES[s.stage]];if(current)s.feedback=current.feedback;return s;}
 async function recordAnswer(session,attempt,preview){
  const p=await get('problems',attempt.problem_id);if(!p)throw new Error('Problem has not been saved on this device.');
  await transaction(['sessions','outbox','attempts','meta'],'readwrite',async tx=>{
   const out=tx.objectStore('outbox');const old=await request(out.get(attempt.id));
   const comparable=a=>JSON.stringify({...a,review:false});
   if(old&&comparable(old.attempt)!==comparable(attempt))throw new Error('Attempt ID is already used by another answer.');
   const positionKey=[p.source_game_id||p.id,p.turn,p.size,p.to_play].join(':');
   const prior=await request(tx.objectStore('sessions').getAll());
   const repeated=prior.some(s=>s.id!==session.id&&(s.problem_id===p.id||s.position_key===positionKey));
   const review=old?.attempt.review??(attempt.review===true||session.review===true||repeated);
   attempt={...attempt,review};preview={...preview,review};session={...session,review,position_key:positionKey};
   session={...session,feedback:preview,results:{...(session.results||{}),[attempt.stage]:{attempt_id:attempt.id,answer:attempt.answer,feedback:preview}},updated:Date.now()};
   await request(tx.objectStore('sessions').put(session));
   await request(out.put({id:attempt.id,attempt,session_id:session.id,preview,source:{size:p.size,black_rank:p.black_rank,white_rank:p.white_rank},sync_error:null}));
   await request(tx.objectStore('meta').put({id:'active-session',value:session.id}));
  });
  return session;
 }
 async function progress(){
  return transaction(['meta','outbox'],'readonly',async tx=>{
   const row=await request(tx.objectStore('meta').get('server-progress'));const result=structuredClone(row?.value||EMPTY());
   result.review||=EMPTY();
   for(const view of [result,result.review])view.candidate_quality={blunders:0,serious_blunders:0,total_sets:0,top_move_hits:0,total_moves:0,total_loss:0,sound_moves:0,average_loss:null,breakdown:[],...view.candidate_quality};
   for(const view of [result,result.review])view.score_quality={total_estimates:0,total_error:0,average_error:null,within_two:0,within_one:0,breakdown:[],...view.score_quality};
   const out=await request(tx.objectStore('outbox').getAll());result.pending_sync=out.length;result.pending_analysis=0;
   for(const entry of out){const f=entry.preview,target=(f.review??entry.attempt.review)?result.review:result;if(f.pending||typeof f.correct!=='boolean'){result.pending_analysis++;continue;}
    if(entry.attempt.stage==='candidates'){
     const q=target.candidate_quality,rank=meanRank(entry.source.black_rank,entry.source.white_rank);
     const moves=(f.candidates||[]).filter(m=>Number.isFinite(m.loss)&&m.loss>=0);
     if(moves.length){let row=q.breakdown.find(r=>r.size===entry.source.size&&r.rank===rank);if(!row){row={...entry.source,rank,total_moves:0,total_loss:0,sound_moves:0,average_loss:null};q.breakdown.push(row);}
      for(const target of [q,row]){target.total_sets=(target.total_sets||0)+1;target.top_move_hits=(target.top_move_hits||0)+Number(f.engine_top_hit??moves.some(m=>m.move&&m.move===f.suggestions?.[0]?.move));target.blunders||=0;target.serious_blunders||=0;}
      for(const move of moves)for(const target of [q,row]){target.total_moves++;target.total_loss+=move.loss;target.sound_moves+=Number(move.loss<2);target.blunders+=Number(move.loss>5);target.serious_blunders+=Number(move.loss>10);target.average_loss=target.total_loss/target.total_moves;}
     }
    }
    if(entry.attempt.stage==='score'&&Number.isFinite(f.error)){
     const q=target.score_quality,rank=meanRank(entry.source.black_rank,entry.source.white_rank);let row=q.breakdown.find(r=>r.size===entry.source.size&&r.rank===rank);if(!row){row={size:entry.source.size,rank,total_estimates:0,total_error:0,within_two:0,within_one:0};q.breakdown.push(row);}for(const t of [q,row]){t.total_estimates++;t.total_error+=f.error;t.average_error=t.total_error/t.total_estimates;t.within_two+=Number(f.error<=2);t.within_one+=Number(f.error<=1);}
    }
    target.total++;const skill=target.skills[entry.attempt.stage]||(target.skills[entry.attempt.stage]={total:0,correct:0});skill.total++;skill.correct+=Number(f.correct);
    const rank=meanRank(entry.source.black_rank,entry.source.white_rank);let group=target.breakdown.find(r=>r.stage===entry.attempt.stage&&r.size===entry.source.size&&r.rank===rank);
    if(!group){group={stage:entry.attempt.stage,size:entry.source.size,rank,total:0,correct:0};target.breakdown.push(group);}group.total++;group.correct+=Number(f.correct);
    target.recent.unshift({stage:entry.attempt.stage,size:entry.source.size,correct:f.correct,created:entry.attempt.created,local:true});
   }
   result.recent=result.recent.slice(0,12);result.review.recent=result.review.recent.slice(0,12);return result;
  });
 }
 async function acceptSync(response,sent){
  const ids=new Set(sent.map(r=>r.id));
  await transaction(['meta','outbox','attempts','sessions'],'readwrite',async tx=>{
   for(const item of response.accepted||[]){if(!ids.has(item.id))continue;const entry=await request(tx.objectStore('outbox').get(item.id));if(!entry)continue;
    await request(tx.objectStore('attempts').put({...entry,feedback:item.feedback,synced_at:Date.now()}));
    const session=await request(tx.objectStore('sessions').get(entry.session_id));
    if(session?.results?.[entry.attempt.stage]?.attempt_id===item.id){session.results[entry.attempt.stage].feedback=item.feedback;if(STAGES[session.stage]===entry.attempt.stage)session.feedback=item.feedback;await request(tx.objectStore('sessions').put(session));}
    await request(tx.objectStore('outbox').delete(item.id));
   }
   for(const item of response.rejected||[]){if(!ids.has(item.id))continue;const entry=await request(tx.objectStore('outbox').get(item.id));if(entry)await request(tx.objectStore('outbox').put({...entry,sync_error:item.error}));}
   if(response.progress){const prior=await request(tx.objectStore('meta').get('server-progress'));if(!prior||response.progress.total>=prior.value.total)await request(tx.objectStore('meta').put({id:'server-progress',value:response.progress}));}
   await request(tx.objectStore('meta').put({id:'last-sync',value:Date.now()}));
  });
 }
 function sync(send){if(inflight)return inflight;inflight=(async()=>{let accepted=[],rejected=[];const tried=new Set();while(true){const entries=(await all('outbox')).filter(e=>!tried.has(e.id)).slice(0,100);if(!entries.length)break;for(const entry of entries)tried.add(entry.id);const response=await send(entries.map(e=>e.attempt));await acceptSync(response,entries);accepted.push(...(response.accepted||[]));rejected.push(...(response.rejected||[]));}return{accepted,rejected,progress:await progress()};})().finally(()=>{inflight=null;});return inflight;}
 return{open,setServerProgress,putProblem,problem:id=>get('problems',id),readyProblems,savePack,saveSession,skipQuestion,activeSession,recordAnswer,pending:()=>all('outbox'),progress,sync,meta,setMeta,packs:()=>all('packs'),sessions:()=>all('sessions'),history:()=>all('attempts')};
}
