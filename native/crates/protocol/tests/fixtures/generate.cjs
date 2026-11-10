const sodium = require('libsodium-wrappers');
const fs = require('node:fs');
const crypto = require('node:crypto');
sodium.ready.then(() => {
  const password = 'interop test password 🔐';
  const kp = {identifier:'fixture@example.invalid',pw_nonce:'92'.repeat(32),version:'004',origination:'registration',created:'1794225600000'};
  const salt = crypto.createHash('sha256').update(kp.identifier + ':' + kp.pw_nonce).digest().subarray(0,16);
  const derived = sodium.crypto_pwhash(64,password,salt,5,67108864,sodium.crypto_pwhash_ALG_DEFAULT);
  const master = Buffer.from(derived).subarray(0,32).toString('hex');
  let nonceCounter=1;
  const b64=v=>sodium.to_base64(JSON.stringify(Object.fromEntries(Object.entries(v).sort(([a],[b])=>a.localeCompare(b)))),sodium.base64_variants.ORIGINAL);
  function enc(plain,key,aad) { const nonce=Buffer.alloc(24,nonceCounter++); const ad=b64(aad); const ciphertext=sodium.crypto_aead_xchacha20poly1305_ietf_encrypt(plain,ad,null,nonce,Buffer.from(key,'hex')); return ['004',nonce.toString('hex'),sodium.to_base64(ciphertext,sodium.base64_variants.ORIGINAL),ad,'e30='].join(':'); }
  const keyId='11111111-1111-4111-8111-111111111111';
  const itemsKey='ab'.repeat(32);
  const raws=[
    {uuid:keyId,content_type:'SN|ItemsKey',content:{itemsKey,version:'004',isDefault:true,creationTimestamp:1794225600000,references:[],appData:{}}},
    {uuid:'22222222-2222-4222-8222-222222222222',content_type:'Note',content:{title:'Nota de interoperabilidade',text:'Olá, Rune. Criptografia real 🔐\nLinha dois.',noteType:'plain-text',appData:{'org.standardnotes.sn':{pinned:true}},references:[],custom:{array:[true,42,'á']}},items_key_id:keyId,extension_metadata:{unfamiliar:true}},
    {uuid:'33333333-3333-4333-8333-333333333333',content_type:'Tag',content:{title:'Pesquisa',references:[{uuid:'22222222-2222-4222-8222-222222222222',content_type:'Note'}],appData:{}},items_key_id:keyId},
    {uuid:'44444444-4444-4444-8444-444444444444',content_type:'SN|Editor',content:{name:'Editor desconhecido',references:[],appData:{},opaque:{version:9,payload:['keep','everything']}},items_key_id:keyId}
  ];
  const items=raws.map(item=>{
    const isKey=item.content_type==='SN|ItemsKey';
    const aad={u:item.uuid,v:'004',...(isKey?{kp}:{})};
    const contentKey='cd'.repeat(32);
    return {...item,created_at:'2026-11-09T12:00:00.000Z',updated_at:'2026-11-09T12:00:00.123Z',created_at_timestamp:1794225600000000,updated_at_timestamp:1794225600123456,deleted:false,enc_item_key:enc(contentKey,isKey?master:itemsKey,aad),content:enc(JSON.stringify(item.content),contentKey,aad)};
  });
  const output=__dirname;
  fs.mkdirSync(output,{recursive:true});
  fs.writeFileSync(output+'/libsodium-004.sn',JSON.stringify({version:'004',keyParams:kp,items},null,2)+'\n');
  fs.writeFileSync(output+'/libsodium-004-expected.json',JSON.stringify({password,master_key:master,server_password:Buffer.from(derived).subarray(32).toString('hex'),items:raws},null,2)+'\n');
  const systemIdentifier='private-vault-fixture';
  const vaultRootKey='ef'.repeat(32), vaultItemsKey='12'.repeat(32), vaultToken='56'.repeat(32);
  const vaultKeyId='66666666-6666-4666-8666-666666666666';
  const vaultParams={systemIdentifier,seed:'34'.repeat(32),creationTimestamp:1794225600000};
  const vaultRaw=[
    {uuid:'55555555-5555-4555-8555-555555555555',content_type:'SN|KeySystemRootKey',content:{systemIdentifier,key:vaultRootKey,keyVersion:'004',token:vaultToken,keyParams:vaultParams,references:[],appData:{}}},
    {uuid:vaultKeyId,content_type:'SN|KeySystemItemsKey',key_system_identifier:systemIdentifier,content:{itemsKey:vaultItemsKey,version:'004',rootKeyToken:vaultToken,creationTimestamp:1794225600000,references:[],appData:{}}},
    {uuid:'77777777-7777-4777-8777-777777777777',content_type:'Note',key_system_identifier:systemIdentifier,items_key_id:vaultKeyId,content:{title:'Nota no cofre privado',text:'Hierarquia de chaves preservada 🔐',noteType:'plain-text',references:[],appData:{}}}
  ];
  const vaultItems=vaultRaw.map(item=>{
    const root=item.content_type==='SN|KeySystemRootKey';
    const vaultKey=item.content_type==='SN|KeySystemItemsKey';
    const aad={u:item.uuid,v:'004',...(item.key_system_identifier?{ksi:systemIdentifier}:{}),...(root?{kp}:vaultKey?{kp:vaultParams}:{})};
    const contentKey='78'.repeat(32);
    return {...item,created_at:'2026-11-09T12:00:00.000Z',updated_at:'2026-11-09T12:00:00.123Z',created_at_timestamp:1794225600000000,updated_at_timestamp:1794225600123456,deleted:false,enc_item_key:enc(contentKey,root?master:vaultKey?vaultRootKey:vaultItemsKey,aad),content:enc(JSON.stringify(item.content),contentKey,aad)};
  });
  fs.writeFileSync(output+'/libsodium-004-private-vault.sn',JSON.stringify({version:'004',keyParams:kp,items:[...items,...vaultItems]},null,2)+'\n');
  process.stdout.write('Generated standard and private-vault protocol 004 fixtures with libsodium-wrappers 0.7.10\n');
});
