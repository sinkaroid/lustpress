# Lustpress na Vercel — arquivos prontos

Copie estes arquivos para a **raiz do seu fork** do [sinkaroid/lustpress](https://github.com/sinkaroid/lustpress).

## Por que dá `FUNCTION_INVOCATION_FAILED`?

| Causa | Detalhe |
|-------|---------|
| `.listen()` no `src/index.ts` | Vercel **não** suporta servidor persistente; precisa de `export default app` |
| Sem `export default` | A função serverless não encontra o handler |
| Runtime Node sem Bun | `Bun.version` em `LustPress.ts` lança `ReferenceError` |
| Sem `vercel.json` | Repo oficial não inclui config Vercel (só Dockerfile) |
| Scraping lento | Timeout padrão 10s (Hobby); buscas podem estourar |

O repositório oficial **não tem** `vercel.json`, `api/` nem `railway.toml` — foi feito para Docker/Railway (processo contínuo).

## Passo a passo (fork)

### 1. Copiar arquivos

```text
seu-fork/
├── vercel.json          ← docs/lustpress-vercel/vercel.json
├── api/
│   └── index.ts         ← docs/lustpress-vercel/api/index.ts
└── src/
    ├── app.ts           ← docs/lustpress-vercel/src/app.ts  (NOVO)
    └── index.ts         ← docs/lustpress-vercel/src/index.ts (SUBSTITUIR)
```

### 2. Patch em `src/LustPress.ts`

Aplique o trecho em `patches/LustPress.ts.snippet` no construtor da classe `LustPress`.

### 3. `package.json` (opcional)

- Adicione `optionalDependencies` de `package.json.additions` se usar **pnpm** (peer deps do Elysia).
- Não remova `"engines": { "bun": ">=1.3.13" }`.

### 4. Variáveis no painel Vercel (projeto Lustpress)

| Variável | Obrigatória | Exemplo |
|----------|-------------|---------|
| `USER_AGENT` | Recomendada | `lustpress/8.3.6 Node.js/22` |
| `EXPIRE_CACHE` | Não | `1` (horas) |
| `REDIS_URL` | Não | `redis://...` (cache entre cold starts) |
| `PORT` | Não na Vercel | Vercel define automaticamente |

**Não** defina `LUSTPRESS_URL` no projeto Lustpress — isso é só no XisVideos.

### 5. Configuração do projeto na Vercel

- **Framework Preset:** Other
- **Root Directory:** `.` (raiz do fork)
- **Install Command:** `bun install` (já no `vercel.json`)
- **Build Command:** vazio
- **Output Directory:** vazio

### 6. Deploy e teste

```bash
curl https://SEU-LUSTPRESS.vercel.app/
# Esperado: JSON com "success": true

curl "https://SEU-LUSTPRESS.vercel.app/xvideos/search?key=sex&page=0"
# Esperado: JSON com lista de vídeos (pode demorar até 60s na 1ª vez)
```

### 7. XisVideos

No projeto **XisVideos** (outro app Vercel):

```env
LUSTPRESS_URL=https://SEU-LUSTPRESS.vercel.app
```

## `maxDuration`

- Hobby: até **60s** (já configurado em `vercel.json`)
- Pro: pode subir para 300s editando `functions.*.maxDuration`

## Se ainda falhar → Railway (recomendado para produção)

Scraping com cold start + timeout é frágil na Vercel. Para uso estável:

1. Use o `Dockerfile` do repo no [Railway](https://railway.app) ou [Render](https://render.com)
2. Defina `USER_AGENT` e opcionalmente `REDIS_URL`
3. Use a URL Railway/Render em `LUSTPRESS_URL` no XisVideos

## Teste local com Vercel CLI

```bash
bun install
npm i -g vercel
vercel dev
# Abra http://localhost:3000/
```
