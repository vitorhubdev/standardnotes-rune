# Protocol 004 interoperability fixtures

These fixtures contain **synthetic public test data only**. They are generated
with `libsodium-wrappers` **0.7.10**, the exact version pinned by the original
`packages/sncrypto-web/package.json`. The password and expected root keys in
`libsodium-004-expected.json` are deliberately public test vectors, never account
credentials.

The generator follows the original implementation's Argon2id parameters, salt,
JSON/base64 authenticated data and XChaCha20Poly1305 IETF format. It produces a
root-encrypted items key plus a note, tag and unfamiliar editor payload. The
timestamps deliberately contain sub-millisecond precision in the numeric field.
The additional private-vault fixture adds the account-root → vault-root →
vault-items-key → note key hierarchy, without shared-vault signatures.

Rust tests must decrypt every item without losing arbitrary content or metadata.
They also exercise wrong passwords, altered ciphertext, UUID substitution,
pagination, missing acknowledgements, partial remote write failures, token
refresh, and conflict preservation against a local HTTP mock. The mock tests do
not establish compatibility with every deployed Standard Notes server or account.

Generate again with Node and `libsodium-wrappers@0.7.10` available on `NODE_PATH`:

```sh
node generate.cjs
```

Reference code in this fork:

- `packages/encryption/src/Domain/Algorithm.ts`
- `packages/encryption/src/Domain/Operator/004/UseCase/RootKey/DeriveRootKey.ts`
- `packages/encryption/src/Domain/Operator/004/UseCase/Symmetric/GenerateEncryptedParameters.ts`
- `packages/sncrypto-web/src/crypto.ts`
- `packages/sncrypto-web/test/crypto.test.js`
- `packages/snjs/lib/Services/Api/ApiService.ts`
- `packages/snjs/mocha/lib/factory.js` (`dateToMicroseconds`)
