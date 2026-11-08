import {grade,validCompleteProblem,STAGES} from './grader.js';
import {createStore} from './offline.js';
import {replayVariation} from './preview.js';
const $=s=>document.querySelector(s);
const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const stages=STAGES;
const names={winner:'Winner judgment',score:'Score estimation',groups:'Ownership assessment',candidates:'Candidate quality'};
const cache=createStore();
let session=null,practice=null,stage=0,candidates=[],busy=false,feedback=null,highlight=null,status=null,serverOnline=false;
let variation=null,variationPinned=false;
let assessments={},thoughts={},preparing=false,refreshing=false,applyingUpdate=false,registration=null;
const letters='ABCDEFGHJKLMNOPQRST';
const uid=()=>crypto.randomUUID?.()||Array.from(crypto.getRandomValues(new Uint8Array(16)),v=>v.toString(16).padStart(2,'0')).join('');
const candidateTarget=()=>Math.min(3,session?.legal_moves?.length||3);
const rankSelect=$('#rank');
for(let i=30;i>=1;i--)rankSelect.add(new Option(`${i} kyu`,`${i}k`));
for(let i=1;i<=9;i++)rankSelect.add(new Option(`${i} dan`,`${i}d`));
try{rankSelect.value=localStorage.getItem('wholeboard-rank')||'10k';}catch{rankSelect.value='10k';}
rankSelect.addEventListener('change',()=>{try{localStorage.setItem('wholeboard-rank',rankSelect.value);}catch{}});
async function api(path,body){
 const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),body?180000:6000);let response;
 try{response=await fetch(path,{signal:controller.signal,...(body?{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{})});serverOnline=true;}
 catch(e){serverOnline=false;throw new Error(e.name==='AbortError'?'Server request timed out. Your saved answers remain on this device.':'The training server is unreachable.');}
 finally{clearTimeout(timer);}
 let result;try{result=await response.json();}catch{throw new Error('Unexpected server response. Use the Rust Whole Board service.');}
 if(!response.ok)throw new Error(result.error||'Could not complete the request.');return result;
}
function error(message){$('#error').textContent=message;$('#error').classList.toggle('hidden',!message);}
function setBusy(value){busy=value;$('#new-position').disabled=value;$('#board-size').disabled=value;$('#rank').disabled=value;$('#game-speed').disabled=value;}
function snapshot(){return {...practice,problem_id:session.id,stage,candidates:[...candidates],assessments:{...assessments},thoughts:{...thoughts},feedback,updated:Date.now()};}
async function remember(){if(!practice||!session)return;try{practice=snapshot();await cache.saveSession(structuredClone(practice));}catch(e){error(`Could not save this session on the device: ${e.message}`);}}
function saveDraft(){if(!practice||feedback)return;practice.draft||={};if(stage===0)practice.draft.winner=$('input[name="winner"]:checked')?.value;if(stage===1)practice.draft.score={side:$('#score-side').value,points:$('#score-points').value};if(stage===2)document.querySelectorAll('[data-assess]').forEach(el=>assessments[el.dataset.assess]=el.value);thoughts[stage]=$('#reasoning')?.value||'';void remember();}
function displayTime(value){const d=typeof value==='number'?new Date(value*1000):new Date(String(value).replace(' ','T')+'Z');return Number.isFinite(d.getTime())?d.toLocaleString('en-US',{timeZone:'America/Chicago',month:'short',day:'numeric',hour:'numeric',minute:'2-digit'}):'';}
function dateLabel(value){const n=Number(value);return Number.isFinite(n)&&n>10000000?new Date(n*1000).toLocaleDateString('en-US',{timeZone:'America/Chicago'}):value||'';}
function scoreLabel(score){return Math.abs(score)<.05?'Even':`${score>0?'Black':'White'} +${Math.abs(score).toFixed(1)}`;}
function winnerLabel(winner){return winner==='even'?'Close · within 2 points':winner==='B'?'Black is ahead':'White is ahead';}
function coordinate(x,y){return letters[x]+(session.size-y);}
function drawBoard(){
 if(!session)return;
 const preview=variation?replayVariation(session,variation.moves,variation.step):null;
 const shown=preview?.board||session.board;
 const n=session.size,pad=34,step=500/(n-1),total=568;
 const at=(x,y)=>[pad+x*step,pad+y*step];
 let svg=`<svg viewBox="0 0 ${total} ${total}" role="group" aria-label="${n} by ${n} Go board. ${stage===3?'Choose three independent candidate moves.':stage===2?'Numbered stones identify connected groups.':'Read the position.'}"><defs><radialGradient id="black-stone" cx="34%" cy="27%"><stop stop-color="#52605a"/><stop offset=".42" stop-color="#2d3633"/><stop offset="1" stop-color="#141d1b"/></radialGradient><radialGradient id="white-stone" cx="33%" cy="24%"><stop stop-color="#fffef8"/><stop offset=".7" stop-color="#f4f1e6"/><stop offset="1" stop-color="#d7d2c4"/></radialGradient><filter id="shadow" x="-30%" y="-30%" width="170%" height="170%"><feDropShadow dx="1" dy="2" stdDeviation="1.1" flood-opacity=".22"/></filter></defs>`;
 for(let i=0;i<n;i++){
  const pos=pad+i*step;
  svg+=`<path d="M${pad} ${pos}H534 M${pos} ${pad}V534" stroke="#785d35" stroke-width="${i===0||i===n-1?1.4:.8}" opacity=".8"/><text x="${pos}" y="17" text-anchor="middle" font-size="10" fill="#785d35">${letters[i]}</text><text x="${pos}" y="556" text-anchor="middle" font-size="10" fill="#785d35">${letters[i]}</text><text x="14" y="${pos+3}" text-anchor="middle" font-size="10" fill="#785d35">${n-i}</text><text x="554" y="${pos+3}" text-anchor="middle" font-size="10" fill="#785d35">${n-i}</text>`;
 }
 const stars=n===9?[2,4,6]:n===13?[3,6,9]:[3,9,15];
 for(const x of stars)for(const y of stars){if(n!==19&&((x===(n-1)/2)!==(y===(n-1)/2)))continue;const [a,b]=at(x,y);svg+=`<circle cx="${a}" cy="${b}" r="2.5" fill="#72552e"/>`;}
 for(let y=0;y<n;y++)for(let x=0;x<n;x++){
  const color=shown[y][x], [a,b]=at(x,y),v=coordinate(x,y),idx=preview?-1:candidates.indexOf(v);
  if(color)svg+=`<circle cx="${a}" cy="${b}" r="${step*.46}" fill="url(#${color===1?'black':'white'}-stone)" filter="url(#shadow)"/>`;
  if(!preview&&v===session.last_move&&stage!==2)svg+=`<circle cx="${a}" cy="${b}" r="${step*.13}" stroke="${color===1?'#f1ead7':'#55604b'}" stroke-width="1.6" fill="none"/>`;
  if(idx>=0)svg+=`<circle cx="${a}" cy="${b}" r="${step*.39}" fill="#254d3b" stroke="#f1f5dc" stroke-width="2"/><text x="${a}" y="${b+step*.13}" font-size="${step*.4}" fill="#fffce9" text-anchor="middle" font-family="system-ui">${idx+1}</text>`;
  svg+=`<circle class="board-point" data-x="${x}" data-y="${y}" cx="${a}" cy="${b}" r="${step*.49}" fill="transparent" tabindex="${x===0&&y===0?'0':'-1'}" role="button" aria-label="${v}, ${color===1?'black stone':color===2?'white stone':'empty intersection'}${idx>=0?', candidate '+(idx+1):''}"/>`;
 }
 if(preview){for(const m of preview.markers){const [a,b]=at(m.x,m.y);svg+=`<text x="${a}" y="${b+step*.13}" text-anchor="middle" font-size="${step*.38}" font-family="system-ui" fill="${m.color===1?'#fff':'#172e25'}" pointer-events="none">${m.number}</text>`;}}
 if(stage===2&&!preview){for(const g of session.groups){
  const [x,y]=g.stones[0],[a,b]=at(x,y);
  if(highlight===g.id)for(const [gx,gy]of g.stones){const[c,d]=at(gx,gy);svg+=`<circle cx="${c}" cy="${d}" r="${step*.48}" fill="none" stroke="#8caa45" stroke-width="3" pointer-events="none"/>`;}
  svg+=`<text x="${a}" y="${b+step*.13}" text-anchor="middle" font-family="system-ui" font-size="${step*.38}" fill="${g.color==='B'?'#e4edbf':'#344e2f'}" pointer-events="none">${g.id}</text>`;
 }}
 svg+='</svg>';$('#board').innerHTML=svg;
 renderVariationControls(preview);
 $('#board').querySelectorAll('.board-point').forEach(point=>{
  point.addEventListener('click',()=>boardClick(Number(point.dataset.x),Number(point.dataset.y)));
  point.addEventListener('keydown',event=>{
   if(['Enter',' '].includes(event.key)){event.preventDefault();point.click();return;}
   const direction={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[event.key];
   if(direction){event.preventDefault();const x=Math.max(0,Math.min(n-1,Number(point.dataset.x)+direction[0])),y=Math.max(0,Math.min(n-1,Number(point.dataset.y)+direction[1]));const next=$(`[data-x="${x}"][data-y="${y}"]`);point.setAttribute('tabindex','-1');next.setAttribute('tabindex','0');next.focus();}
  });
 });
}
function boardClick(x,y){
 if(busy||feedback)return;
 if(stage===3&&!session.board[y][x]){
  if(!session.legal_moves.includes(coordinate(x,y))){error('That move is not legal in this position.');return;}
  const move=coordinate(x,y),index=candidates.indexOf(move);
  if(index>=0)candidates.splice(index,1);else if(candidates.length<candidateTarget())candidates.push(move);
  drawBoard();renderSlots();void remember();
 }else if(stage===2&&session.board[y][x]){
  const g=session.groups.find(g=>g.stones.some(([a,b])=>a===x&&b===y));
  highlight=g.id;drawBoard();const row=$(`#group-${g.id}`);row.scrollIntoView({behavior:'smooth',block:'nearest'});row.querySelector('select').focus();
 }
}
function renderSlots(){
 const el=$('#candidate-slots');if(!el)return;
 el.innerHTML=Array.from({length:candidateTarget()},(_,i)=>`<div class="candidate-slot ${candidates[i]?'filled':''}">${esc(candidates[i]||'＋')}<small>CANDIDATE ${i+1}</small></div>`).join('');
 const submit=$('#submit');if(submit)submit.disabled=candidates.length!==candidateTarget();
 const pass=$('#pass-candidate');if(pass){const included=candidates.includes('pass');pass.textContent=included?'Remove pass':'Include pass';pass.setAttribute('aria-pressed',String(included));pass.disabled=!included&&candidates.length===candidateTarget();}

}
function renderQuestion(){
 variation=null;variationPinned=false;
 $('#steps').querySelectorAll('li').forEach((li,i)=>{li.className=practice?.skipped?.includes(stages[i])?'skipped':i<stage?'done':i===stage?'active':'';li.title=practice?.skipped?.includes(stages[i])?'Skipped · not scored':'';});
 $('#step-count').textContent=`${Math.min(stage+1,4)} / 4`;
 let body='';
 if(stage===0){body=`<span class="question-number">01 / THE BIG PICTURE</span><h2 class="question-title">Who has the better position?</h2><p class="question-description">Take in the whole board. Include komi, weak groups, and the initiative. “Close” means within 2 points.</p><div class="choices">${[['B','Black is ahead','stone-icon'],['W','White is ahead','stone-icon white'],['even','Too close to call','even-icon']].map(([v,label,cls])=>`<label class="choice"><span class="${cls}">${v==='even'?'≈':''}</span>${label}<input type="radio" name="winner" value="${v}" required></label>`).join('')}</div>`;}
 else if(stage===1){body=`<span class="question-number">02 / MAKE IT CONCRETE</span><h2 class="question-title">Put a number on it.</h2><p class="question-description">Estimate the final lead if both players play well. Include ${session.komi} points of komi for White. Within 3 points earns credit.</p><div class="field-label">WHO LEADS, AND BY HOW MANY POINTS?</div><div class="score-inputs"><select id="score-side" aria-label="Leading color"><option value="B">Black leads</option><option value="W">White leads</option></select><input id="score-points" aria-label="Lead in points" type="number" min="0" max="${session.size**2+100}" step="0.5" placeholder="e.g. 5.5" required></div>`;}
 else if(stage===2){body=`<span class="question-number">03 / READ THE GROUPS</span><h2 class="question-title">Which groups look secure?</h2><p class="question-description">Assess each connected group. Click a stone to find its row. We compare ownership estimates; they cannot prove life or death.</p><div class="group-list">${session.groups.map(g=>`<div class="group-row" id="group-${g.id}"><button type="button" data-group="${g.id}" aria-label="Highlight group ${g.id}">${g.id}</button><label for="assessment-${g.id}">${g.color==='B'?'●':'○'} ${esc(g.anchor)} · ${g.stones.length} stone${g.stones.length>1?'s':''}</label><select id="assessment-${g.id}" data-assess="${g.id}" required><option value="">Your assessment</option>${['secure-looking','uncertain','at-risk'].map(v=>`<option value="${v}" ${assessments[g.id]===v?'selected':''}>${v==='secure-looking'?'Secure-looking':v==='at-risk'?'At risk':'Uncertain'}</option>`).join('')}</select></div>`).join('')}</div>`;}
 else{body=`<span class="question-number">04 / TURN READING INTO ACTION</span><h2 class="question-title">Find ${candidateTarget()} sound ${candidateTarget()===1?'move':'moves'}.</h2><p class="question-description">${session.to_play==='B'?'Black':'White'} to play. Choose empty intersections on the board, or include pass. Each of the ${candidateTarget()} candidates is a separate choice, not a sequence. Click again to remove.</p><div id="candidate-slots" class="candidate-slots"></div><button type="button" class="button" id="pass-candidate" style="margin-top:12px" aria-pressed="false">Include pass</button><p class="question-description" style="margin-top:15px">Every candidate can earn credit, including alternatives to the engine’s favorites. Legal moves within 2 estimated points of the baseline count as sound.</p>`;}
 $('#question').innerHTML=`<form id="answer-form">${body}<label class="thought-label" for="reasoning">YOUR REASONING <span style="opacity:.65">· OPTIONAL, SAVED ON THIS DEVICE</span></label><textarea id="reasoning" class="thought-input" maxlength="20000" rows="2" placeholder="What makes you think so?">${esc(thoughts[stage]||'')}</textarea><button class="button primary submit-answer" id="submit" type="submit">Check my judgment <span>↗</span></button><button class="button skip-question" id="skip-question" type="button">${stage===3?'Skip and finish position':'Skip question →'}</button><div id="feedback" aria-live="polite"></div></form>`;
 $('#answer-form').addEventListener('submit',submitAnswer);
 $('#skip-question').addEventListener('click',skipQuestion);
 $('#reasoning').addEventListener('input',e=>{thoughts[stage]=e.target.value;});
 document.querySelectorAll('[data-group]').forEach(btn=>btn.addEventListener('click',()=>{highlight=Number(btn.dataset.group);drawBoard();}));
 document.querySelectorAll('[data-assess]').forEach(el=>el.addEventListener('change',()=>{assessments[el.dataset.assess]=el.value;}));
 const passButton=$('#pass-candidate');if(passButton)passButton.addEventListener('click',()=>{
  if(busy||feedback)return;const index=candidates.indexOf('pass');
  if(index>=0)candidates.splice(index,1);else if(candidates.length<candidateTarget())candidates.push('pass');
  renderSlots();drawBoard();void remember();
 });
 $('#answer-form').addEventListener('input',saveDraft);
 $('#answer-form').addEventListener('change',saveDraft);
 if(stage===0&&practice?.draft?.winner){const chosen=document.querySelector(`input[name="winner"][value="${practice.draft.winner}"]`);if(chosen)chosen.checked=true;}
 if(stage===1&&practice?.draft?.score){$('#score-side').value=practice.draft.score.side;$('#score-points').value=practice.draft.score.points;}
 renderSlots();drawBoard();
 if(feedback){$('#answer-form').querySelectorAll('input,select,textarea').forEach(el=>el.disabled=true);$('#submit').classList.add('hidden');$('#skip-question').classList.add('hidden');renderFeedback();}
}
async function skipQuestion(){
 if(busy||feedback)return;setBusy(true);error('');
 try{
  const next=await cache.skipQuestion(snapshot());
  practice=next;stage=next.stage;feedback=null;highlight=null;
  if(next.finished){
   $('#question').innerHTML='<h2 class="question-title">Position finished.</h2><p class="question-description">Skipped questions do not affect your accuracy.</p><button class="button primary" id="next-position" type="button">Practice another position →</button>';
   $('#next-position').addEventListener('click',newPosition);drawBoard();
  }else renderQuestion();
 }catch(e){error(`Question was not skipped: ${e.message}`);}
 finally{setBusy(false);}
}
async function submitAnswer(event){
 event.preventDefault();if(busy||feedback)return;
 let answer;
 if(stage===0){const chosen=$('input[name="winner"]:checked');if(!chosen)return;answer=chosen.value;}
 if(stage===1)answer=Number($('#score-points').value)*($('#score-side').value==='B'?1:-1);
 if(stage===2)answer={...assessments};if(stage===3)answer=[...candidates];
 setBusy(true);error('');
 try{
  const preview=grade(session,stages[stage],answer);
  const attempt={id:uid(),problem_id:session.id,stage:stages[stage],answer,reasoning:thoughts[stage]||'',created:Math.floor(Date.now()/1000),grader_version:session.analysis.grader_version};
  practice=await cache.recordAnswer(snapshot(),attempt,preview);feedback=preview;
  renderQuestion();await refreshProgress();void syncNow(true);
 }catch(e){error(`Answer was not saved: ${e.message}`);}
 finally{setBusy(false);}
}
function renderFeedback(){
 const f=feedback;let detail='';
 if(stage===0)detail=`<p><strong>${winnerLabel(f.expected)}.</strong></p>`;
 if(stage===1)detail=`<div class="score-reveal">${scoreLabel(f.score)}</div><p>Your estimate was ${f.error.toFixed(1)} points away.</p>`;
 if(stage===2)detail=`<p>${f.groups.filter(g=>g.correct).length} / ${f.groups.length} ownership assessments agreed.</p><ul>${f.groups.map(g=>`<li>${g.correct?'✓':'↳'} Group ${g.id} (${esc(g.anchor)}): ${esc(g.expected)}</li>`).join('')}</ul>`;
 if(stage===3)detail=`<ul>${f.candidates.map(c=>`<li><strong>${esc(c.move)}</strong> · ${c.correct?'sound at this search depth':'reconsider'} · ${c.loss.toFixed(1)} estimated points lost<br><span style="opacity:.7">Engine continuation: ${(c.pv||[]).map(esc).join(' → ')} ${(c.pv||[]).length?`<button class="button pv-button" type="button" data-pv="candidate-${esc(c.move)}" aria-pressed="false">Preview ${esc(c.move)} line</button>`:''}</span></li>`).join('')}</ul>${f.pending?`<p>Waiting for server analysis: ${f.missing.map(esc).join(', ')}. These moves have not been marked wrong.</p>`:`<p>Engine suggestions: ${f.suggestions.map(m=>`<button class="button pv-button" type="button" data-pv="suggestion-${esc(m.move)}" aria-pressed="false">${esc(m.move)}${m.pv?.length?' · preview':''}</button>`).join(' ')}.</p>`}`;
 $('#feedback').innerHTML=`<div class="feedback ${f.pending?'pending-feedback':f.correct?'':'needs-work'}"><h3>${f.pending?'◷ Saved; analysis pending.':f.correct?'✓ Your read holds up.':'↳ A useful thing to notice.'}</h3>${detail}<p>${esc(f.message)}</p><button class="button primary" type="button" id="continue">${stage===3?'Practice another position':'Continue to '+['','counting','group assessment','candidate moves'][stage+1]} <span>→</span></button></div>`;
 bindVariationPreviews();
 $('#continue').addEventListener('click',async()=>{if(busy)return;if(stage===3){practice.finished=true;await remember();await newPosition();return;}stage++;feedback=null;highlight=null;await remember();renderQuestion();});
}
function bindVariationPreviews(){
 if(stage!==3)return;
 for(const button of document.querySelectorAll('[data-pv]')){
  const key=button.dataset.pv,split=key.indexOf('-'),type=key.slice(0,split),move=key.slice(split+1);
  const line=(type==='candidate'?feedback.candidates:feedback.suggestions).find(c=>c.move===move);
  if(!line?.pv?.length){button.disabled=true;continue;}
  const show=()=>{variation={key,label:move,moves:line.pv,step:line.pv.length};drawBoard();};
  const restore=event=>{if(event?.relatedTarget?.closest?.('#variation-controls'))return;if(!variationPinned&&variation?.key===key){variation=null;drawBoard();}};
  button.addEventListener('pointerenter',event=>{if(event.pointerType!=='touch'&&!variationPinned)show();});
  button.addEventListener('pointerleave',restore);button.addEventListener('focus',()=>{if(!variationPinned)show();});button.addEventListener('blur',restore);
  button.addEventListener('click',()=>{if(variationPinned&&variation?.key===key){variationPinned=false;variation=null;drawBoard();}else{variationPinned=true;show();}for(const b of document.querySelectorAll('[data-pv]'))b.setAttribute('aria-pressed',String(variationPinned&&b.dataset.pv===variation?.key));});
 }
}
function renderVariationControls(preview){
 let controls=$('#variation-controls');if(!controls){controls=document.createElement('div');controls.id='variation-controls';$('#board').after(controls);}
 if(!variation){controls.innerHTML='';return;}
 controls.innerHTML=`<div class="variation-controls"><strong>Reference line: ${esc(variation.label)} · ${preview.played} / ${variation.moves.length} moves</strong><p>Numbered stones show move order.${preview.passes.length?' Pass: '+preview.passes.map(p=>p.number).join(', ')+'.':''}${preview.error?' '+esc(preview.error):''}</p><div><button class="button" type="button" id="variation-back" ${variation.step===0?'disabled':''}>← Previous</button><button class="button" type="button" id="variation-next" ${variation.step===variation.moves.length?'disabled':''}>Next →</button><button class="button" type="button" id="variation-close">Original position</button></div></div>`;
 controls.onpointerdown=()=>{variationPinned=true;};
 const restore=event=>{if(!variationPinned&&!controls.contains(event.relatedTarget)){variation=null;drawBoard();}};
 controls.onpointerleave=restore;controls.onfocusout=restore;
 const step=delta=>{variationPinned=true;variation.step=Math.max(0,Math.min(variation.moves.length,variation.step+delta));drawBoard();};
 $('#variation-back').addEventListener('click',()=>step(-1));$('#variation-next').addEventListener('click',()=>step(1));
 $('#variation-close').addEventListener('click',()=>{variation=null;variationPinned=false;drawBoard();for(const b of document.querySelectorAll('[data-pv]'))b.setAttribute('aria-pressed','false');});
}
function showProblem(p,s){
 variation=null;variationPinned=false;session=p;practice=s;stage=s.stage||0;candidates=[...(s.candidates||[])];assessments={...(s.assessments||{})};thoughts={...(s.thoughts||{})};feedback=s.feedback||null;highlight=null;
 $('#position-kind').textContent=p.sample?'INTRODUCTORY POSITION · CONSTRUCTED':validCompleteProblem(p)?'DOWNLOADED POSITION · OFFLINE READY':'HISTORICAL GAME · SERVER ANALYSIS';
 $('#position-title').textContent=`Move ${p.turn} · ${p.to_play==='B'?'Black':'White'} to play`;
 $('#board-badge').textContent=`${p.size} × ${p.size}`;
 $('#position-meta').innerHTML=`<span><strong>● ${esc(p.sample?'Black':p.black)}</strong> · ${esc(p.black_rank)}</span><span><strong>○ ${esc(p.sample?'White':p.white)}</strong> · ${esc(p.white_rank)}</span><span>${esc(p.rules)} · Komi ${p.komi}${p.handicap?' · '+p.handicap+' handicap stones':''}</span>`;
 $('#source-note').textContent=`All ranks included · ${p.speed||'Unknown speed'} · ${dateLabel(p.date)}. ${validCompleteProblem(p)?'Every legal candidate is available for offline grading.':'Unsearched candidates are analyzed when the server is reachable.'} Accuracy is tracked by the recorded Black/White ranks.`;
 const prefix='https://online-go.com/game/',gamePart=String(p.source_url||'').slice(prefix.length);
 if(String(p.source_url||'').startsWith(prefix)&&gamePart.length&&[...gamePart].every(c=>c>='0'&&c<='9')){const a=document.createElement('a');a.href=p.source_url;a.target='_blank';a.rel='noopener noreferrer';a.textContent='View original OGS game ↗';a.style.cssText='display:block;margin-top:6px;text-decoration:underline';$('#source-note').append(a);}
 if(p.rank_provenance){const note=document.createElement('span');note.className='rank-provenance';note.textContent=p.rank_provenance;$('#source-note').append(note);}
 $('#new-position').innerHTML='Another position <span>↗</span>';renderQuestion();
}
function speedMatches(p,speed){return speed==='all'||p.speed===speed||(speed==='realtime'&&['live','blitz','rapid'].includes(p.speed));}
async function cachedNext(){const size=$('#board-size').value,speed=$('#game-speed').value;let all=(await cache.readyProblems()).filter(p=>(size==='mixed'||String(p.size)===size)&&speedMatches(p,speed));if(!all.length)throw new Error('No matching offline-ready problems on this device. Connect to the server and download a pack.');const history=await cache.sessions();const used=new Set(history.map(s=>s.problem_id));const fresh=all.filter(p=>!used.has(p.id));if(fresh.length)all=fresh;else if(all.length>1)all=all.filter(p=>p.id!==session?.id);return all[Math.floor(Math.random()*all.length)];}
async function newPosition(){
 if(busy)return;await remember();setBusy(true);error('');const button=$('#new-position');button.textContent='Preparing a position…';
 try{
  let p;
  if(navigator.onLine&&serverOnline){try{p=await api('/api/new',{size:$('#board-size').value,speed:$('#game-speed').value});await cache.putProblem(p);}catch(e){p=await cachedNext();$('#sync-message').textContent=`Using downloaded practice. ${e.message}`;}}
  else p=await cachedNext();
  const s={id:uid(),problem_id:p.id,stage:0,candidates:[],assessments:{},thoughts:{},draft:{},feedback:null,results:{},created:Date.now(),finished:false};
  await cache.saveSession(s);showProblem(p,s);
 }catch(e){error(e.message);button.textContent='Try starting again';}
 finally{setBusy(false);await renderConnection();}
}
async function renderConnection(){
 const ready=(await cache.readyProblems()).length;const pending=await cache.pending();
 $('#engine-status').textContent=`${serverOnline?'● Server online':'○ Offline'} · ${ready} ready on this device`;
 $('#offline-summary').textContent=`${ready} problems available offline · ${pending.length} answers awaiting sync`;
 $('#offline-summary').classList.toggle('offline',!serverOnline);
 $('#prepare-pack').disabled=preparing||!serverOnline;
}
function renderProgress(){
 if(!status)return;const p=status.progress;
 $('#progress-content').innerHTML=`<div class="stats-grid">${Object.entries(p.skills).map(([key,value])=>{const rate=value.total?Math.round(value.correct/value.total*100):null;return `<div class="stat-card"><span class="eyebrow">${key==='candidates'?'ALL CANDIDATES SOUND':names[key].toUpperCase()}</span><div class="metric">${rate===null?'—':rate+'%'}</div><small>${value.correct} / ${value.total} assessments</small><div class="metric-track"><i style="width:${rate||0}%"></i></div></div>`;}).join('')}</div><p class="intro-text">${p.pending_sync||0} answers await sync. ${p.pending_analysis||0} need server analysis and are excluded from accuracy until graded.</p><h2 class="section-label">BY BOARD SIZE AND ORIGINAL GAME RANKS</h2>${p.total?`<div class="table-wrap"><table><thead><tr><th>Skill</th><th>Board</th><th>Recorded ranks (B / W)</th><th>Agreement / success</th><th>Attempts</th></tr></thead><tbody>${p.breakdown.map(r=>`<tr><td>${names[r.stage]}</td><td>${r.size} × ${r.size}</td><td>${esc(r.rank)}</td><td>${Math.round(r.correct/r.total*100)}%</td><td>${r.total}</td></tr>`).join('')}</tbody></table></div><h2 class="section-label">RECENT PRACTICE</h2><div class="table-wrap"><table><thead><tr><th>When (Chicago time)</th><th>Skill</th><th>Board</th><th>Result</th></tr></thead><tbody>${p.recent.map(r=>`<tr><td>${esc(displayTime(r.created))}${r.local?' · local':''}</td><td>${names[r.stage]}</td><td>${r.size} × ${r.size}</td><td>${r.correct?'✓ Agreed / sound':'↳ Review'}</td></tr>`).join('')}</tbody></table></div>`:'<div class="library-card"><h2>Your first data point is waiting.</h2><p>Answers are saved on this device before feedback appears, including while offline.</p></div>'}<p class="intro-text">Group assessment measures agreement with ownership estimates, not proven life and death. Candidate quality uses a 2-point tolerance; score estimation uses 3 points. Recorded archive ranks may be snapshots rather than verified ranks at the time of play.</p>`;
 $('#library-content').innerHTML=`<div class="stats-grid">${[9,13,19].map(n=>`<div class="stat-card"><span class="eyebrow">${n} × ${n} INDEXED GAMES</span><div class="metric">${status.sizes?.[n]||0}</div><small>All recorded ranks</small></div>`).join('')}</div><p class="intro-text">${status.raw_records||0} raw JSON records retained. ${status.games||0} games are indexed for supported-board practice. The original source archive is preserved. Select the game speed in the training room.</p><div class="table-wrap"><table><thead><tr><th>Board</th><th>Speed</th><th>Games</th></tr></thead><tbody>${(status.cohorts||[]).map(c=>`<tr><td>${c.size} × ${c.size}</td><td>${esc(c.speed||'Unknown')}</td><td>${c.count}</td></tr>`).join('')}</tbody></table></div><p class="intro-text">Engine: ${esc(status.model||'Connect to configure an engine')}.</p>`;
 const q=p.candidate_quality||{total_moves:0,sound_moves:0,average_loss:null,breakdown:[]};
 const avg=v=>v===null||v===undefined?'—':Number(v).toFixed(2),rate=(sound,n)=>n?Math.round(100*sound/n)+'%':'—';
 $('#progress-content').insertAdjacentHTML('afterbegin',`<section class="candidate-metrics"><h2 class="section-label">CANDIDATE POINT LOSS</h2><div class="stats-grid"><div class="stat-card"><span class="eyebrow">AVERAGE POINTS LOST / MOVE</span><div class="metric">${avg(q.average_loss)}</div><small>${q.total_moves} evaluated candidate moves</small></div><div class="stat-card"><span class="eyebrow">SOUND CANDIDATES</span><div class="metric">${rate(q.sound_moves,q.total_moves)}</div><small>${q.sound_moves} / ${q.total_moves} within 2 points</small></div></div><p class="intro-text">Each proposed move counts separately. These are alternative choices from the same position, not moves played in a game. Skipped questions and answers awaiting analysis are excluded. Rank pairs are recorded Black / White ranks.</p>${q.breakdown.length?`<div class="table-wrap"><table><thead><tr><th>Board</th><th>Recorded ranks (B / W)</th><th>Average loss / move</th><th>Sound moves</th><th>Moves measured</th></tr></thead><tbody>${q.breakdown.map(r=>`<tr><td>${r.size} × ${r.size}</td><td>${esc(r.rank)}</td><td>${avg(r.average_loss)} points</td><td>${rate(r.sound_moves,r.total_moves)}</td><td>${r.total_moves}</td></tr>`).join('')}</tbody></table></div>`:'<p class="intro-text">Submit candidate moves to start measuring point loss.</p>'}</section>`);

}
async function refreshProgress(){if(!status)status={sizes:{},cohorts:[]};status.progress=await cache.progress();renderProgress();await renderConnection();}
async function loadStatus(quiet=false){
 try{const current=await api('/api/status');if(current.api_version!==2)throw new Error('This phone app requires the Rust Whole Board service.');status=current;await cache.setMeta('server-status',current);await cache.setServerProgress(current.progress);}
 catch(e){serverOnline=false;status=await cache.meta('server-status')||{sizes:{},cohorts:[]};if(!quiet&&!session)$('#sync-message').textContent=e.message;}
 await refreshProgress();await renderOffline();
}
async function syncNow(quiet=false){
 if(!quiet)$('#sync-message').textContent='Syncing saved answers…';
 try{
  const result=await cache.sync(attempts=>api('/api/sync',{attempts}));
  if(practice){for(const accepted of result.accepted){for(const item of Object.values(practice.results||{})){if(item.attempt_id===accepted.id){item.feedback=accepted.feedback;if(accepted.feedback.stage===stages[stage]&&feedback){feedback=accepted.feedback;renderFeedback();}}}}}
  if(result.rejected.length)$('#sync-message').textContent=`${result.rejected.length} answers remain saved here: ${result.rejected[0].error}`;
  else if(!quiet||result.accepted.length)$('#sync-message').textContent=`${result.accepted.length} answers synced. Other saved answers remain queued.`;
 }catch(e){if(!quiet)$('#sync-message').textContent=`Answers remain on this device. ${e.message}`;}
 await refreshProgress();
}
async function subscribePack(id){const subscribed=await cache.meta('subscribed-packs')||[];if(!subscribed.includes(id))await cache.setMeta('subscribed-packs',[...subscribed,id]);}
async function downloadPack(id){const response=await api(`/api/packs/${encodeURIComponent(id)}`);await cache.savePack(response);await refreshProgress();return response;}
async function preparePack(){
 if(preparing)return;preparing=true;await renderConnection();
 try{
  let request=await cache.meta('pending-pack-request');
  if(!request){request={id:uid(),count:Number($('#pack-count').value),size:$('#board-size').value,speed:$('#game-speed').value};await cache.setMeta('pending-pack-request',request);}
  const job=await api('/api/packs',request);await subscribePack(job.id);await cache.setMeta('pending-pack-request',null);
  $('#sync-message').textContent='Pack queued. The server will search every legal candidate before each problem becomes available offline.';
  await loadStatus(true);
 }catch(e){$('#sync-message').textContent=`Pack request saved for retry. ${e.message}`;}
 finally{preparing=false;await renderConnection();}
}
async function renderOffline(){
 const ready=await cache.readyProblems(),pending=await cache.pending(),cachedPacks=await cache.packs();
 $('#offline-count').textContent=String(ready.length);$('#outbox-count').textContent=String(pending.length);
 const jobs=status?.jobs||[];
 $('#pack-list').innerHTML=jobs.length?jobs.map(j=>{const local=cachedPacks.find(p=>p.id===j.id);return `<article class="library-card pack-card"><div><span class="eyebrow">${esc(j.state.toUpperCase())}</span><h2>${j.completed} / ${j.requested} problems prepared</h2><p>${esc(j.size==='mixed'?'Mixed board sizes':j.size+' × '+j.size)} · ${esc(j.speed)} · ${local?.problem_ids.length||0} downloaded here</p>${j.current_moves_total?`<p>Current position: ${j.current_moves_done} / ${j.current_moves_total} legal moves evaluated.</p>`:''}${j.error?`<p class="job-error">${esc(j.error)}</p>`:''}</div><div class="pack-actions">${j.completed?`<button class="button" data-download-pack="${esc(j.id)}">Download ready problems</button>`:''}${['failed','cancelled'].includes(j.state)?`<button class="button" data-retry-pack="${esc(j.id)}">Retry preparation</button>`:''}${['queued','running'].includes(j.state)?`<button class="button" data-cancel-pack="${esc(j.id)}">Cancel preparation</button>`:''}</div></article>`;}).join(''):'<div class="library-card"><h2>A little practice to take with you.</h2><p>Prepare a pack while the server is reachable. Its problems stay on this phone for practice away from the server.</p></div>';
 document.querySelectorAll('[data-download-pack]').forEach(b=>b.addEventListener('click',async()=>{try{await subscribePack(b.dataset.downloadPack);await downloadPack(b.dataset.downloadPack);await renderOffline();}catch(e){error(`Could not save the pack: ${e.message}`);}}));
 for(const action of ['retry','cancel'])document.querySelectorAll(`[data-${action}-pack]`).forEach(b=>b.addEventListener('click',async()=>{try{await api(`/api/packs/${encodeURIComponent(b.dataset[action+'Pack'])}/${action}`,{});await loadStatus(true);}catch(e){error(e.message);}}));
}
async function pollPacks(){
 if(refreshing||!serverOnline||document.visibilityState!=='visible')return;refreshing=true;
 try{
  const response=await api('/api/packs');if(!status)status={};status.jobs=response.jobs;
  const subscribed=await cache.meta('subscribed-packs')||[],local=await cache.packs();
  for(const j of response.jobs){if(subscribed.includes(j.id)&&j.completed>(local.find(p=>p.id===j.id)?.problem_ids.length||0))await downloadPack(j.id);}
  await cache.setMeta('server-status',status);await renderOffline();
 }catch(e){$('#sync-message').textContent=`Pack data already downloaded remains available. ${e.message}`;await renderConnection();}
 finally{refreshing=false;}
}
async function setupServiceWorker(){
 if(!('serviceWorker'in navigator)||!window.isSecureContext){$('#storage-message').textContent='Phone installation and offline launch require HTTPS. Localhost is supported for development.';return;}
 try{
  registration=await navigator.serviceWorker.register('/sw.js');
  function waiting(){if(registration.waiting)$('#update-app').classList.remove('hidden');}
  waiting();registration.addEventListener('updatefound',()=>{registration.installing?.addEventListener('statechange',waiting);});
  navigator.serviceWorker.addEventListener('controllerchange',()=>{if(applyingUpdate)location.reload();});
 }catch(e){$('#storage-message').textContent=`Offline app-shell setup failed: ${e.message}`;}
}
$('#new-position').addEventListener('click',newPosition);$('#prepare-pack').addEventListener('click',preparePack);$('#sync-now').addEventListener('click',()=>loadStatus(true).then(()=>syncNow()));
$('#persist-storage').addEventListener('click',async()=>{try{const granted=await navigator.storage?.persist?.();$('#storage-message').textContent=granted?'Persistent storage is enabled for this device.':'The browser did not grant persistent storage. Current offline availability is shown above.';}catch(e){$('#storage-message').textContent=e.message;}});
$('#update-app').addEventListener('click',async()=>{await remember();applyingUpdate=true;registration?.waiting?.postMessage({type:'SKIP_WAITING'});});
document.querySelectorAll('[data-view]').forEach(button=>button.addEventListener('click',()=>{
 const view=button.dataset.view;document.querySelectorAll('[data-view]').forEach(b=>b.classList.toggle('active',b===button));
 for(const v of ['practice','progress','library','offline'])$(`#${v}-view`).classList.toggle('hidden',view!==v);
 $('#page-name').textContent={practice:'Whole-board judgment',progress:'Your progress',library:'Game library',offline:'Offline practice'}[view];error('');
 void loadStatus(true);
}));
$('#import-sgf').addEventListener('click',()=>$('#sgf-file').click());$('#sgf-file').addEventListener('change',async e=>{
 const file=e.target.files[0];if(!file)return;if(file.size>5000000){error('Choose an SGF under 5 MB.');return;}
 try{await api('/api/import',{filename:file.name,sgf:await file.text()});$('#import-message').textContent='Imported. Supported positions are now available for practice.';await loadStatus(true);}catch(error){$('#import-message').textContent=error.message;}e.target.value='';
});
window.addEventListener('offline',()=>{serverOnline=false;void renderConnection();});
window.addEventListener('online',()=>{void loadStatus(true).then(()=>syncNow(true));});
window.addEventListener('focus',()=>{void loadStatus(true).then(()=>syncNow(true));});
setInterval(()=>void pollPacks(),5000);
async function boot(){
 try{await cache.open();const active=await cache.activeSession();if(active&&!active.finished){const p=await cache.problem(active.problem_id);if(p)showProblem(p,active);}await setupServiceWorker();await loadStatus();void pollPacks();void syncNow(true);}
 catch(e){error(`Offline storage is unavailable: ${e.message}. Answers cannot be saved safely on this device.`);}
}
void boot();
