// Native browser proof against an isolated LOCAL server/database only.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {execFileSync}=require('node:child_process');
const base=process.env.REVIEW_TEST_BASE_URL||'http://127.0.0.1:8087';
const db=process.env.DATABASE_URL;
assert(['localhost','127.0.0.1','[::1]'].includes(new URL(base).hostname));
assert(db&&['localhost','127.0.0.1','[::1]'].includes(new URL(db).hostname),'Local DB required');
const output=process.env.ADMIN_TEST_OUTPUT||'/tmp/chantels-admin-browser';fs.mkdirSync(output,{recursive:true});
const password='AdminBrowser27!Unique';const stamp=Date.now();
const sql=query=>execFileSync('psql',['-d',db,'-Atc',query],{encoding:'utf8'}).trim();
const catalogTables=['books','authors','book_authors','genres','book_genres','book_copies','variant_attributes','book_collections','book_collection_items','book_subjects'];
const fingerprint=()=>Object.fromEntries(catalogTables.map(t=>[t,sql(`SELECT md5(COALESCE(string_agg(row_to_json(t)::text,'' ORDER BY row_to_json(t)::text),'')) FROM ${t} t`)]));
// Optional cleanup only for an explicitly isolated local test database.
if(process.env.REVIEW_TEST_RESET_RATE_LIMITS==='1')sql('DELETE FROM account_email_rate_limits');
const before=fingerprint();
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.REVIEW_TEST_CHROME||chromium.executablePath()});const contexts=[];
 try {
  const account=async name=>{
   const ctx=await browser.newContext({viewport:{width:1280,height:900},javaScriptEnabled:false,reducedMotion:'reduce'});contexts.push(ctx);const page=await ctx.newPage();
   const email=`admin-browser-${name.toLowerCase()}-${stamp}@example.com`;
   await page.goto(`${base}/signup`);await page.getByLabel('First name',{exact:true}).fill(name);await page.getByLabel('Last name',{exact:true}).fill('Reader');await page.getByLabel('Email address',{exact:true}).fill(email);await page.getByLabel('Password',{exact:true}).fill(password);await page.getByLabel('Re-enter password',{exact:true}).fill(password);await page.getByRole('button',{name:'Continue',exact:true}).click();
   assert.equal(await page.locator('.auth-alert').count(),0,'Signup failed');await page.waitForURL(url=>url.pathname==='/account/profile');
   return {page,email,id:sql(`SELECT id FROM users WHERE email='${email}'`)};
  };
  const login=async a=>{await a.page.goto(`${base}/login`);await a.page.getByLabel('Email address',{exact:true}).fill(a.email);await a.page.getByLabel('Password',{exact:true}).fill(password);await a.page.getByRole('button',{name:'Sign In',exact:true}).click();await a.page.waitForURL(url=>url.pathname==='/');};
  const token=async a=>{await a.page.goto(`${base}/account/profile`);return a.page.locator('input[name=csrf]').first().inputValue();};
  const post=async(a,target,data)=>a.page.request.post(`${base}/staff/users/${target.id}`,{headers:{Origin:base},form:{csrf:await token(a),...data}});
  const grant=async(owner,target,role)=>{
   await owner.page.goto(`${base}/staff/users/${target.id}`);
   const form=owner.page.locator('form').filter({has:owner.page.locator(`input[name=role][value="${role}"]`)});
   await form.locator('input[name=reason]').fill(`Assign ${role} for local browser proof`);await form.getByRole('button',{name:'Grant',exact:true}).click();await owner.page.waitForURL(url=>url.pathname===`/staff/users/${target.id}`);
  };
  const owner=await account('Owner');const customer=await account('Customer');const support=await account('Support');const moderator=await account('Moderator');const rolesOnly=await account('RoleAdmin');
  assert.equal((await customer.page.request.get(`${base}/staff/users`)).status(),403);
  execFileSync(path.resolve('target/debug/chantels-corner'),['--bootstrap-admin',owner.email,'browser-operator','Bootstrap isolated browser owner'],{env:process.env,stdio:'pipe'});
  assert.equal((await owner.page.request.get(`${base}/staff/users`)).status(),401,'Bootstrap failed to revoke initial session');await login(owner);
  // Retire only prior synthetic browser-owner support grants through the real UI,
  // ensuring this run has exactly one capable admin for the invariant check.
  for(const id of sql(`SELECT u.id FROM users u JOIN user_staff_roles r ON r.user_id=u.id AND r.role='support' WHERE u.email LIKE 'admin-browser-owner-%' AND u.id<>'${owner.id}'`).split('\n').filter(Boolean)) {
   await owner.page.goto(`${base}/staff/users/${id}`);
   const form=owner.page.locator('form').filter({has:owner.page.locator('input[name=role][value="support"]')});
   await form.locator('input[name=reason]').fill('Retire support grant from prior isolated browser fixture');await form.getByRole('button',{name:'Revoke',exact:true}).click();
  }
  await owner.page.goto(`${base}/staff/users`);await owner.page.getByLabel('Search name, email or account ID',{exact:true}).fill(customer.email);await owner.page.getByRole('button',{name:'Search',exact:true}).click();await owner.page.getByRole('link',{name:customer.email,exact:true}).click();
  const profile=owner.page.locator('form').filter({has:owner.page.locator('input[name=action][value=profile]')});
  await profile.locator('input[name=first_name]').fill('Updated Customer');await profile.locator('input[name=phone_number]').fill('555-0102');await profile.locator('input[name=address_city]').fill('La Habra');await profile.locator('input[name=reason]').fill('Customer requested profile correction');const staleVersion=await profile.locator('input[name=version]').inputValue();await profile.getByRole('button',{name:'Save profile',exact:true}).click();
  await owner.page.waitForURL(url=>url.pathname===`/staff/users/${customer.id}`);
  // The customer's still-open signup/profile form is stale after the staff edit.
  const staleCustomerResponse=customer.page.waitForResponse(r=>r.url().endsWith('/account/profile')&&r.request().method()==='POST');
  await customer.page.locator('form[action="/account/profile"] button[type=submit]').click();
  assert.equal((await staleCustomerResponse).status(),409,'Stale customer profile overwrote staff correction');
  await customer.page.goto(`${base}/account/profile`);assert.equal(await customer.page.locator('input[name=first_name]').inputValue(),'Updated Customer');
  const stale=await post(owner,customer,{action:'profile',version:staleVersion,first_name:'Stale overwrite',last_name:'Reader',reason:'Competing stale profile'});assert.equal(stale.status(),409);
  const status=async(next)=>{
   await owner.page.goto(`${base}/staff/users/${customer.id}`);const form=owner.page.locator('form').filter({has:owner.page.locator('input[name=action][value=status]')});await form.locator('select[name=status]').selectOption(next);await form.locator('input[name=reason]').fill(`Customer account ${next} test`);await form.getByRole('button',{name:'Update access',exact:true}).click();await owner.page.waitForURL(url=>url.pathname===`/staff/users/${customer.id}`);
  };
  await status('disabled');await customer.page.goto(`${base}/account/profile`);assert(new URL(customer.page.url()).pathname==='/login');
  await customer.page.getByLabel('Email address',{exact:true}).fill(customer.email);await customer.page.getByLabel('Password',{exact:true}).fill(password);await customer.page.getByRole('button',{name:'Sign In',exact:true}).click();assert(new URL(customer.page.url()).pathname==='/login','Disabled login succeeded');
  await status('active');await login(customer);
  await owner.page.goto(`${base}/staff/users/${customer.id}`);const sessions=owner.page.locator('form').filter({has:owner.page.locator('input[name=action][value=sessions]')});await sessions.locator('input[name=reason]').fill('Revoke compromised customer sessions');await sessions.getByRole('button',{name:'Revoke sessions',exact:true}).click();await customer.page.goto(`${base}/account/profile`);assert(new URL(customer.page.url()).pathname==='/login');
  await grant(owner,support,'support');await login(support);await support.page.goto(`${base}/staff/users`);assert(await support.page.getByRole('heading',{name:'User administration',exact:true}).count());
  await support.page.goto(`${base}/staff/users/${owner.id}`);assert.equal(await support.page.getByRole('button',{name:'Save profile',exact:true}).count(),0);
  assert.equal((await post(support,owner,{action:'status',status:'disabled',reason:'Attempt unauthorized staff disable'})).status(),403);
  assert.equal((await post(support,customer,{action:'grant',role:'administrator',reason:'Attempt unsupported promotion'})).status(),403);
  await grant(owner,moderator,'moderator');await login(moderator);assert.equal((await moderator.page.request.get(`${base}/staff/reviews`)).status(),200);assert.equal((await moderator.page.request.get(`${base}/staff/users`)).status(),403);
  await grant(owner,rolesOnly,'administrator');await login(rolesOnly);await rolesOnly.page.goto(`${base}/staff/users/${customer.id}`);assert.equal(await rolesOnly.page.getByRole('button',{name:'Save profile',exact:true}).count(),0);assert(await rolesOnly.page.getByRole('heading',{name:'Staff access',exact:true}).count());
  assert.equal((await post(rolesOnly,customer,{action:'profile',version:'0',first_name:'Forged edit',reason:'No profile permission'})).status(),403);
  // Former staff audit is still privileged after the target's roles are removed.
  await grant(owner,customer,'moderator');
  await owner.page.goto(`${base}/staff/users/${customer.id}`);
  const retire=owner.page.locator('form').filter({has:owner.page.locator('input[name=role][value="moderator"]')});
  await retire.locator('input[name=reason]').fill('Retire former staff grant for audit boundary');await retire.getByRole('button',{name:'Revoke',exact:true}).click();
  await support.page.goto(`${base}/staff/users/${customer.id}`);
  const supportAudit=await support.page.locator('main').innerText();
  assert(supportAudit.includes('Customer requested profile correction'));
  assert(!supportAudit.includes('Retire former staff grant for audit boundary'));
  assert(!supportAudit.includes('Assign moderator for local browser proof'));
  await rolesOnly.page.goto(`${base}/staff/users/${customer.id}`);
  const staffAudit=await rolesOnly.page.locator('main').innerText();
  assert(staffAudit.includes('Retire former staff grant for audit boundary'));
  assert(!staffAudit.includes('Customer requested profile correction'));
  assert.equal((await post(owner,owner,{action:'grant',role:'moderator',reason:'Attempt own role escalation'})).status(),400);
  assert.equal((await post(owner,owner,{action:'status',status:'disabled',reason:'Attempt disable last capable admin'})).status(),400);
  // Role-only admin cannot remove the support capability from the only capable owner.
  assert.equal((await post(rolesOnly,owner,{action:'revoke',role:'support',reason:'Attempt revoke last admin support'})).status(),400);
  await owner.page.goto(`${base}/staff/users/${customer.id}`);assert((await owner.page.locator('main').innerText()).includes('Customer requested profile correction'));
  await owner.page.screenshot({path:path.join(output,'user-detail-desktop.png'),fullPage:true});
  await owner.page.setViewportSize({width:390,height:844});await owner.page.screenshot({path:path.join(output,'user-detail-mobile.png'),fullPage:true});assert(await owner.page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'Mobile detail overflow');
  await owner.page.goto(`${base}/staff/users`);await owner.page.screenshot({path:path.join(output,'users-mobile.png'),fullPage:true});assert(await owner.page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'Mobile directory overflow');
  assert.deepEqual(fingerprint(),before,'User admin modified catalog records');
  console.log(JSON.stringify({result:'PASS',flows:['normal signup denied staff','bootstrap revokes old session','account search','profile correction','stale profile rejection','disable/login block','reenable/fresh login','forced session revocation','staff-role assignment','support-only staff denial','moderator isolation','role-only admin UI','self-role prohibition','last-capable-admin protection','audit timeline and independent event permissions','no-JavaScript native forms','desktop/mobile layout','unchanged catalog fingerprints'],screenshots:output},null,2));
 } finally {for(const c of contexts)await c.close();await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
