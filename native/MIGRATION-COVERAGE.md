# Cobertura da migração para Rune

## Escopo e referência

- Origem: `standardnotes/app`, commit `f64d5a4a1991d0e5d1c326c6131673ab4b72684c`.
- UI nativa: `anomalyco/rune`, commit `e970367bd7d2a51e2e7d58e2978ea32eac7780cb`.
- Alvos desta implementação: desktop Windows x64 e Linux x64.
- Estágio: **prévia de migração; paridade integral ainda não atingida**.

A origem contém aproximadamente 217.850 linhas em arquivos JavaScript/TypeScript
nos pacotes, incluindo testes. A interface React e os serviços dependentes de DOM
não podem ser compilados diretamente pelo Rune. A pasta `native` é a nova
implementação Rust; o código original é conservado para referência, sem fazer
parte do executável nativo.

## Contratos de dados

- Campos desconhecidos de itens e conteúdos de editores não suportados devem
  sobreviver ao ciclo de importação, armazenamento e exportação.
- O editor de texto só altera formatos reconhecidos. Ele não transforma
  documentos Lexical ou extensões HTML em texto simples.
- A importação de um UUID com conteúdo divergente cria uma cópia de conflito,
  preservando a versão existente. Duplicatas equivalentes são deduplicadas.
- O cofre local é um formato próprio autenticado (`.snrvault`), com Argon2id e
  XChaCha20-Poly1305; ele não é o banco IndexedDB do Electron.
- Arquivo principal, temporário de gravação e backup do cofre guardam somente
  dados criptografados. Exportações `.sn` sem senha são explicitamente legíveis.
- O cofre possui limite de 256 MiB. Anexos binários do serviço Standard Notes não
  são copiados automaticamente para ele.

## Matriz de compatibilidade

Esta matriz distingue implementação, teste local e paridade com o upstream.
Os resultados concretos de execução são registrados na entrega e no CI.

| Área | Implementação nativa | Limite de paridade |
|---|---|---|
| Janela e renderização | Rune/wgpu, X11/Wayland e Windows | Dependente de drivers gráficos suportados pelo Rune |
| Notas simples | Criar, editar título/texto, buscar, salvar, bloquear | Sem todas as ferramentas avançadas do editor original |
| Organização | Etiquetas, fixadas, arquivo, lixeira | Smart views e hierarquia completa exigem port adicional |
| Markdown | Edição de texto simples e visualização nativa | Não substitui os editores externos nem converte formatos opacos |
| Cofre local | Senha, bloqueio entre processos, salvamento atômico, backup | Formato próprio; sem importação direta do perfil Electron |
| Backup legível | Importação/exportação `.sn`, `.txt`, `.md` | Formatos desconhecidos preservados, sem execução de extensões |
| Backup com senha | Importação/exportação de documentos `.sn` do protocolo 004, sem perder metadados | Vetores com libsodium do upstream; não aceita protocolos legados nem itens compartilhados assinados |
| Protocolo 004 | Argon2id, XChaCha20-Poly1305, chaves de itens e hierarquia de cofres privados | Interoperabilidade cruzada Rust/JavaScript; não cobre todas as gerações do protocolo |
| Conta e sincronização | Login com PKCE/TOTP, sessão no cofre criptografado, sincronização manual, paginação, renovação e cópias de conflito | HTTP validado com servidor simulado; nenhuma conta real foi utilizada nesta entrega |
| Windows/Linux | Workflow de testes, build e empacotamento | Sem assinatura Windows; não inclui ARM64/macOS/mobile |
| Super Editor/Lexical | Conteúdo preservado | Edição estruturada ainda não portada |
| Extensões/planilhas/iframes | Conteúdo e metadados preservados | Runtime HTML/DOM não existe no Rune |
| Anexos e mídia | Metadados conservados quando presentes no backup | Upload/download, preview e secretstream ainda não portados |
| Cofres compartilhados | Payloads assinados não suportados são rejeitados antes de aplicar a sincronização | Assinaturas, compartilhamento e rotação compartilhada pendentes; cofres privados têm suporte às chaves |
| Protocolos antigos 001–003 | Rejeição explícita de payloads criptografados não suportados | Não reimplementar parcialmente nem tentar adivinhar chaves |
| Integrações de conta | Sessão e protocolo separados da UI | Passkeys, registro, pagamentos e gerenciamento de plano pendentes |
| Integrações de desktop | Diálogos de arquivos e atalhos nativos | Tray, updater, home-server e spellcheck completo pendentes |

## Critérios antes de declarar paridade completa

1. Portar editores estruturados e validar documentos existentes sem perdas.
2. Validar protocolos legados, sistemas de chaves, arquivos e compartilhamento.
3. Executar a matriz de sincronização com servidor de teste e contas de teste,
   incluindo conflitos, tombstones, múltiplos dispositivos e expiração de sessão.
4. Completar as integrações de desktop e as configurações que fazem parte dos
   fluxos originais.
5. Executar testes de uso nas plataformas-alvo, incluindo fechar imediatamente
   após editar, falha de gravação, senha incorreta e reabertura do cofre.

Uma compilação bem-sucedida não elimina essas pendências. Nenhum artefato desta
branch deve ser descrito como uma migração integral concluída enquanto a matriz
mantiver recursos pendentes.

## Verificação executável

`cargo test --workspace --locked` cobre os contratos de itens, importação sem
perdas, preservação de conflitos, gravação criptografada, bloqueio entre
processos, recuperação e troca de senha do cofre. Os testes do protocolo usam
fixtures sintéticas produzidas com `libsodium-wrappers` da versão utilizada pelo
upstream e um servidor HTTP local controlado, incluindo expiração HTTP 498 e
rejeição de conteúdo remoto não autenticado. Esses testes não equivalem a uso
com o serviço público ou uma conta real.

Os testes Rune acionam os controles de criação/edição, atalhos, navegação
compacta, conta e backup, além de simular callbacks de seletores de arquivos.
Há regressões para confirmação atrasada de salvamento, falha de autosave
durante diálogo, bloqueio de edição durante sincronização e fechamento enquanto
há operações pendentes. A cena `demo` e o teste visual ignorado por padrão
produzem capturas por meio do renderizador do Rune.

As instruções e licenças de terceiros acompanham os pacotes em `third-party/`.
O inventário deriva do `Cargo.lock` e separa dependências de runtime de
ferramentas executadas durante a compilação.
