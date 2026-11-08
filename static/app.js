'use strict';
const $=s=>document.querySelector(s);
const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const stages=['winner','score','groups','candidates'];
const names={winner:'Winner judgment',score:'Score estimation',groups:'Ownership assessment',candidates:'Candidate quality'};
let session=null,stage=0,candidates=[],busy=false,feedback=null,highlight=null,status=null;
let assessments={},thoughts={};
const letters='ABCDEFGHJKLMNOPQRST';
const rankSelect=$('#rank');
for(let i=30;i>=1;i--)rankSelect.add(new Option(`${i} kyu`,`${i}k`));
for(let i=1;i<=9;i++)rankSelect.add(new Option(`${i} dan`,`${i}d`));
try{rankSelect.value=localStorage.getItem('wholeboard-rank')||'10k';}catch{rankSelect.value='10k';}
rankSelect.addEventListener('change',()=>{try{localStorage.setItem('wholeboard-rank',rankSelect.value);}catch{}});
async function api(path,body){
 const response=await fetch(path,body?{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{});
 const result=await response.json();
 if(!response.ok)throw new Error(result.error||'Could not complete the request.');
 return result;
}
function error(message){$('#error').textContent=message;$('#error').classList.toggle('hidden',!message);}
function setBusy(value){busy=value;$('#new-position').disabled=value;$('#board-size').disabled=value;$('#rank').disabled=value;$('#sgf-file').disabled=value;}
function scoreLabel(score){return Math.abs(score)<.05?'Even':`${score>0?'Black':'White'} +${Math.abs(score).toFixed(1)}`;}
function winnerLabel(winner){return winner==='even'?'Close · within 2 points':winner==='B'?'Black is ahead':'White is ahead';}
function coordinate(x,y){return letters[x]+(session.size-y);}
function drawBoard(){
 if(!session)return;
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
  const color=session.board[y][x], [a,b]=at(x,y),v=coordinate(x,y),idx=candidates.indexOf(v);
  if(color)svg+=`<circle cx="${a}" cy="${b}" r="${step*.46}" fill="url(#${color===1?'black':'white'}-stone)" filter="url(#shadow)"/>`;
  if(v===session.last_move&&stage!==2)svg+=`<circle cx="${a}" cy="${b}" r="${step*.13}" stroke="${color===1?'#f1ead7':'#55604b'}" stroke-width="1.6" fill="none"/>`;
  if(idx>=0)svg+=`<circle cx="${a}" cy="${b}" r="${step*.39}" fill="#254d3b" stroke="#f1f5dc" stroke-width="2"/><text x="${a}" y="${b+step*.13}" font-size="${step*.4}" fill="#fffce9" text-anchor="middle" font-family="system-ui">${idx+1}</text>`;
  svg+=`<circle class="board-point" data-x="${x}" data-y="${y}" cx="${a}" cy="${b}" r="${step*.49}" fill="transparent" tabindex="${x===0&&y===0?'0':'-1'}" role="button" aria-label="${v}, ${color===1?'black stone':color===2?'white stone':'empty intersection'}${idx>=0?', candidate '+(idx+1):''}"/>`;
 }
 if(stage===2){for(const g of session.groups){
  const [x,y]=g.stones[0],[a,b]=at(x,y);
  if(highlight===g.id)for(const [gx,gy]of g.stones){const[c,d]=at(gx,gy);svg+=`<circle cx="${c}" cy="${d}" r="${step*.48}" fill="none" stroke="#8caa45" stroke-width="3" pointer-events="none"/>`;}
  svg+=`<text x="${a}" y="${b+step*.13}" text-anchor="middle" font-family="system-ui" font-size="${step*.38}" fill="${g.color==='B'?'#e4edbf':'#344e2f'}" pointer-events="none">${g.id}</text>`;
 }}
 svg+='</svg>';$('#board').innerHTML=svg;
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
  const move=coordinate(x,y),index=candidates.indexOf(move);
  if(index>=0)candidates.splice(index,1);else if(candidates.length<3)candidates.push(move);
  drawBoard();renderSlots();
 }else if(stage===2&&session.board[y][x]){
  const g=session.groups.find(g=>g.stones.some(([a,b])=>a===x&&b===y));
  highlight=g.id;drawBoard();const row=$(`#group-${g.id}`);row.scrollIntoView({behavior:'smooth',block:'nearest'});row.querySelector('select').focus();
 }
}
function renderSlots(){
 const el=$('#candidate-slots');if(!el)return;
 el.innerHTML=[0,1,2].map((_,i)=>`<div class="candidate-slot ${candidates[i]?'filled':''}">${esc(candidates[i]||'＋')}<small>CANDIDATE ${i+1}</small></div>`).join('');
 const submit=$('#submit');if(submit)submit.disabled=candidates.length!==3;
 const pass=$('#pass-candidate');if(pass){const included=candidates.includes('pass');pass.textContent=included?'Remove pass':'Include pass';pass.setAttribute('aria-pressed',String(included));pass.disabled=!included&&candidates.length===3;}

}
function renderQuestion(){
 $('#steps').querySelectorAll('li').forEach((li,i)=>{li.className=i<stage?'done':i===stage?'active':'';});
 $('#step-count').textContent=`${Math.min(stage+1,4)} / 4`;
 let body='';
 if(stage===0){body=`<span class="question-number">01 / THE BIG PICTURE</span><h2 class="question-title">Who has the better position?</h2><p class="question-description">Take in the whole board. Include komi, weak groups, and the initiative. “Close” means within 2 points.</p><div class="choices">${[['B','Black is ahead','stone-icon'],['W','White is ahead','stone-icon white'],['even','Too close to call','even-icon']].map(([v,label,cls])=>`<label class="choice"><span class="${cls}">${v==='even'?'≈':''}</span>${label}<input type="radio" name="winner" value="${v}" required></label>`).join('')}</div>`;}
 else if(stage===1){body=`<span class="question-number">02 / MAKE IT CONCRETE</span><h2 class="question-title">Put a number on it.</h2><p class="question-description">Estimate the final lead if both players play well. Include ${session.komi} points of komi for White. Within 3 points earns credit.</p><div class="field-label">WHO LEADS, AND BY HOW MANY POINTS?</div><div class="score-inputs"><select id="score-side" aria-label="Leading color"><option value="B">Black leads</option><option value="W">White leads</option></select><input id="score-points" aria-label="Lead in points" type="number" min="0" max="${session.size**2+100}" step="0.5" placeholder="e.g. 5.5" required></div>`;}
 else if(stage===2){body=`<span class="question-number">03 / READ THE GROUPS</span><h2 class="question-title">Which groups look secure?</h2><p class="question-description">Assess each connected group. Click a stone to find its row. We compare ownership estimates; they cannot prove life or death.</p><div class="group-list">${session.groups.map(g=>`<div class="group-row" id="group-${g.id}"><button type="button" data-group="${g.id}" aria-label="Highlight group ${g.id}">${g.id}</button><label for="assessment-${g.id}">${g.color==='B'?'●':'○'} ${esc(g.anchor)} · ${g.stones.length} stone${g.stones.length>1?'s':''}</label><select id="assessment-${g.id}" data-assess="${g.id}" required><option value="">Your assessment</option>${['secure-looking','uncertain','at-risk'].map(v=>`<option value="${v}" ${assessments[g.id]===v?'selected':''}>${v==='secure-looking'?'Secure-looking':v==='at-risk'?'At risk':'Uncertain'}</option>`).join('')}</select></div>`).join('')}</div>`;}
 else{body=`<span class="question-number">04 / TURN READING INTO ACTION</span><h2 class="question-title">Find three sound moves.</h2><p class="question-description">${session.to_play==='B'?'Black':'White'} to play. Choose empty intersections on the board, or include pass. Each of the three candidates is a separate choice, not a sequence. Click again to remove.</p><div id="candidate-slots" class="candidate-slots"></div><button type="button" class="button" id="pass-candidate" style="margin-top:12px" aria-pressed="false">Include pass</button><p class="question-description" style="margin-top:15px">All three can earn credit, even if none matches the engine’s top three. Legal moves within 2 estimated points of the baseline count as sound.</p>`;}
 $('#question').innerHTML=`<form id="answer-form">${body}<label class="thought-label" for="reasoning">YOUR REASONING <span style="opacity:.65">· OPTIONAL, SAVED ON THIS DEVICE</span></label><textarea id="reasoning" class="thought-input" rows="2" placeholder="What makes you think so?">${esc(thoughts[stage]||'')}</textarea><button class="button primary submit-answer" id="submit" type="submit">Check my judgment <span>↗</span></button><div id="feedback" aria-live="polite"></div></form>`;
 $('#answer-form').addEventListener('submit',submitAnswer);
 $('#reasoning').addEventListener('input',e=>{thoughts[stage]=e.target.value;});
 document.querySelectorAll('[data-group]').forEach(btn=>btn.addEventListener('click',()=>{highlight=Number(btn.dataset.group);drawBoard();}));
 document.querySelectorAll('[data-assess]').forEach(el=>el.addEventListener('change',()=>{assessments[el.dataset.assess]=el.value;}));
 const passButton=$('#pass-candidate');if(passButton)passButton.addEventListener('click',()=>{
  if(busy||feedback)return;const index=candidates.indexOf('pass');
  if(index>=0)candidates.splice(index,1);else if(candidates.length<3)candidates.push('pass');
  renderSlots();drawBoard();
 });
 renderSlots();drawBoard();
}
async function submitAnswer(event){
 event.preventDefault();if(busy||feedback)return;
 let answer;
 if(stage===0){const chosen=$('input[name="winner"]:checked');if(!chosen)return;answer=chosen.value;}
 if(stage===1){const points=Number($('#score-points').value);answer=points*($('#score-side').value==='B'?1:-1);}
 if(stage===2)answer=assessments;
 if(stage===3)answer=[...candidates];
 setBusy(true);error('');
 const submit=$('#submit');submit.disabled=true;submit.innerHTML='<span class="loading-ring"></span> Reading with KataGo…';
 $('#answer-form').querySelectorAll('input,select,textarea').forEach(el=>el.disabled=true);
 try{
  feedback=await api('/api/answer',{session:session.id,stage:stages[stage],answer});
  try{localStorage.setItem(`wholeboard-notes-${session.id}`,JSON.stringify(thoughts));}catch{}
  submit.classList.add('hidden');renderFeedback();await loadStatus();
 }catch(e){error(e.message);submit.disabled=false;submit.innerHTML='Try checking again ↗';$('#answer-form').querySelectorAll('input,select,textarea').forEach(el=>el.disabled=false);}
 finally{setBusy(false);}
}
function renderFeedback(){
 const f=feedback;
 let detail='';
 if(stage===0)detail=`<p><strong>${winnerLabel(f.expected)}.</strong></p>`;
 if(stage===1)detail=`<div class="score-reveal">${scoreLabel(f.score)}</div><p>Your estimate was ${f.error.toFixed(1)} points away.</p>`;
 if(stage===2)detail=`<p>${f.groups.filter(g=>g.correct).length} / ${f.groups.length} ownership assessments agreed.</p><ul>${f.groups.map(g=>`<li>${g.correct?'✓':'↳'} Group ${g.id} (${esc(g.anchor)}): ${esc(g.expected)}</li>`).join('')}</ul>`;
 if(stage===3)detail=`<ul>${f.candidates.map(c=>`<li><strong>${esc(c.move)}</strong> · ${c.correct?'sound at this search depth':'reconsider'} · ${c.loss.toFixed(1)} estimated points lost<br><span style="opacity:.7">Engine continuation: ${c.pv.map(esc).join(' → ')}</span></li>`).join('')}</ul><p>Engine suggestions: ${f.suggestions.map(m=>esc(m.move)).join(', ')}.</p>`;
 $('#feedback').innerHTML=`<div class="feedback ${f.correct?'':'needs-work'}"><h3>${f.correct?'✓ Your read holds up.':'↳ A useful thing to notice.'}</h3>${detail}<p>${esc(f.message)}</p><button class="button primary" type="button" id="continue">${stage===3?'Practice another position':'Continue to '+['','counting','group assessment','candidate moves'][stage+1]} <span>→</span></button></div>`;
 $('#continue').addEventListener('click',()=>{if(stage===3){newPosition();return;}stage++;feedback=null;highlight=null;renderQuestion();});
}
async function newPosition(){
 if(busy)return;setBusy(true);error('');const btn=$('#new-position');btn.textContent='Preparing a position…';
 try{
  session=await api('/api/new',{size:$('#board-size').value,rank:rankSelect.value});stage=0;candidates=[];feedback=null;highlight=null;assessments={};thoughts={};
  $('#position-kind').textContent=session.sample?'INTRODUCTORY POSITION · CONSTRUCTED':'HISTORICAL LIVE GAME';
  $('#position-title').textContent=`Move ${session.turn} · ${session.to_play==='B'?'Black':'White'} to play`;
  $('#board-badge').textContent=`${session.size} × ${session.size}`;
  $('#position-meta').innerHTML=`<span><strong>● ${esc(session.sample?'Black':session.black)}</strong>${session.sample?'':' · '+esc(session.black_rank)}</span><span><strong>○ ${esc(session.sample?'White':session.white)}</strong>${session.sample?'':' · '+esc(session.white_rank)}</span><span>${esc(session.rules)} · Komi ${session.komi}</span>`;
  $('#source-note').textContent=session.sample?'Constructed practice position, not an OGS game. Feedback uses real KataGo analysis. Import verified live SGFs in Game library.':`All player ranks included. Accuracy is tracked separately by the recorded game ranks. Source: ${session.source}${session.date?' · '+session.date:''}`;
  if(!session.sample && /^https:\/\/online-go\.com\/game\/\d+$/.test(session.source_url||'')){
   const link=document.createElement('a');link.href=session.source_url;link.target='_blank';link.rel='noopener noreferrer';
   link.textContent='View original OGS game ↗';link.style.cssText='display:block;margin-top:6px;text-decoration:underline';
   $('#source-note').append(link);
  }
  renderQuestion();btn.innerHTML='Another position <span>↗</span>';
 }catch(e){error(e.message);btn.textContent='Try starting again';}
 finally{setBusy(false);}
}
function renderProgress(){
 if(!status)return;
 const p=status.progress;
 $('#progress-content').innerHTML=`<div class="stats-grid">${Object.entries(p.skills).map(([key,value])=>{const rate=value.total?Math.round(value.correct/value.total*100):null;return `<div class="stat-card"><span class="eyebrow">${names[key].toUpperCase()}</span><div class="metric">${rate===null?'—':rate+'%'}</div><small>${value.correct} / ${value.total} assessments</small><div class="metric-track"><i style="width:${rate||0}%"></i></div></div>`;}).join('')}</div><h2 class="section-label">BY BOARD SIZE AND SOURCE RANK</h2>${p.total?`<div class="table-wrap"><table><thead><tr><th>Skill</th><th>Board</th><th>Source ranks (B / W)</th><th>Agreement / success</th><th>Attempts</th></tr></thead><tbody>${p.breakdown.map(r=>`<tr><td>${names[r.stage]}</td><td>${r.size} × ${r.size}</td><td>${esc(r.rank)}</td><td>${Math.round(r.correct/r.total*100)}%</td><td>${r.total}</td></tr>`).join('')}</tbody></table></div><h2 class="section-label">RECENT PRACTICE</h2><div class="table-wrap"><table><thead><tr><th>When (Chicago time)</th><th>Skill</th><th>Board</th><th>Result</th></tr></thead><tbody>${p.recent.map(r=>`<tr><td>${esc(new Date(r.created.replace(' ','T')+'Z').toLocaleString('en-US',{timeZone:'America/Chicago',month:'short',day:'numeric',hour:'numeric',minute:'2-digit'}))}</td><td>${names[r.stage]}</td><td>${r.size} × ${r.size}</td><td>${r.correct?'✓ Agreed / sound':'↳ Review'}</td></tr>`).join('')}</tbody></table></div>`:'<div class="library-card"><h2>Your first data point is waiting.</h2><p>Complete a question in the training room. Results will appear here and remain after restarting.</p></div>'}<p class="intro-text">Group assessment measures agreement with ownership estimates, not proven life and death. Candidate quality uses a 2-point tolerance; score estimation uses 3 points. Small samples fluctuate.</p>`;
 $('#library-content').innerHTML=`<div class="stats-grid">${[9,13,19].map(n=>`<div class="stat-card"><span class="eyebrow">${n} × ${n} LIVE GAMES</span><div class="metric">${status.sizes[n]}</div><small>Verified live-game metadata</small></div>`).join('')}</div><p class="intro-text">${status.games?'Imported games are available for training by board size.':'No historical games imported yet. Training uses clearly labeled constructed introductory positions.'} Directory, zip, and tar import is also available through the local command line; see README.md.</p><p class="intro-text">KataGo model: ${esc(status.model)}. ${status.engine?'Installed locally.':'Not found; configure KATAGO_MODEL before grading.'}</p>`;
}
async function loadStatus(){
 status=await api('/api/status');$('#engine-status').innerHTML=`<i></i> ${status.engine?'KataGo · local analysis':'KataGo model missing'}`;renderProgress();
}
$('#new-position').addEventListener('click',newPosition);
document.querySelectorAll('[data-view]').forEach(btn=>btn.addEventListener('click',()=>{
 const view=btn.dataset.view;document.querySelectorAll('[data-view]').forEach(b=>b.classList.toggle('active',b===btn));
 ['practice','progress','library'].forEach(v=>$(`#${v}-view`).classList.toggle('hidden',view!==v));
 $('#page-name').textContent={practice:'Whole-board judgment',progress:'Your progress',library:'Game library'}[view];
 error('');loadStatus().catch(e=>error(e.message));
}));
$('#import-sgf').addEventListener('click',()=>$('#sgf-file').click());
$('#sgf-file').addEventListener('change',async event=>{
 const file=event.target.files[0];if(!file)return;
 if(file.size>5000000){error('Choose an SGF smaller than 5 MB.');return;}
 $('#import-message').textContent='Reading and validating the game…';
 try{await api('/api/import',{filename:file.name,sgf:await file.text()});$('#import-message').textContent='Imported. This game is now available in the training room.';await loadStatus();}
 catch(e){$('#import-message').textContent=e.message;}
 event.target.value='';
});
loadStatus().catch(e=>error(e.message));
