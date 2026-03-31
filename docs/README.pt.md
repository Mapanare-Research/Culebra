<div align="center">

# Culebra

**/koo-LEH-brah/**

**Diagnosticos de compilador para linguagens auto-hospedadas que geram LLVM.**

*ABI. IR. Binario. Bootstrap. Um unico binario detecta o que nenhum depurador consegue.*

Nasceu do bootstrap do [Mapanare](https://github.com/Mapanare-Research/Mapanare), onde cada bug era um misterio sem rede de seguranca. Culebra inclui um motor de templates estilo Nuclei para que cada bug de compilador que voce sobreviver se torne um padrao que ninguem mais precise depurar.

[English](../README.md) | [Espanol](README.es.md) | [中文版](README.zh-CN.md) | Portugues

<br>

![Rust](https://img.shields.io/badge/Rust-Edicao_2021-dea584?style=for-the-badge&logo=rust&logoColor=white)
![LLVM](https://img.shields.io/badge/LLVM-Analise_IR-262D3A?style=for-the-badge&logo=llvm&logoColor=white)
![Plataforma](https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-grey?style=for-the-badge)

[![Licenca](https://img.shields.io/badge/licenca-MIT-green.svg?style=flat-square)](../LICENSE)
[![Versao](https://img.shields.io/badge/versao-0.1.0-blue.svg?style=flat-square)](../Cargo.toml)
[![Templates](https://img.shields.io/badge/templates-17_incluidos-orange.svg?style=flat-square)]()
[![GitHub Stars](https://img.shields.io/github/stars/Mapanare-Research/Culebra?style=flat-square&color=f5c542)](https://github.com/Mapanare-Research/Culebra/stargazers)

<br>

[Por que Culebra?](#por-que-culebra) · [Instalar](#instalar) · [Inicio Rapido](#inicio-rapido) · [Motor de Templates](#motor-de-templates) · [Todos os Comandos](#todos-os-comandos) · [Templates Incluidos](#templates-incluidos) · [Configuracao](#configuracao-culebratoml) · [Arquitetura](#arquitetura) · [Documentacao](../docs.md) · [Contribuir](#contribuir)

</div>

---

## Por que Culebra?

A maioria das linguagens se auto-hospedou sobre um compilador maduro:

- **Rust** comecou em OCaml antes de se auto-hospedar cerca de um ano depois.
- **Go** foi escrito em C ate v1.5, depois usou um tradutor automatico de C para Go.
- **C++** se auto-hospedou atraves do Cfront, que traduzia C++ para C.

[Mapanare](https://github.com/Mapanare-Research/Mapanare) nao tem esse luxo. E uma linguagem compilada AI-nativa que gera LLVM IR, construindo seu proprio backend do zero: lexer, AST, inferencia de tipos, emissao de LLVM IR. O compilador bootstrap (Stage 0) e escrito em Python, mas nao ha um compilador maduro por baixo como suporte.

Isso significa que cada incompatibilidade de ABI, cada erro de contagem de bytes em strings, cada divergencia de layout de structs entre IR e C, cada regressao de estagio de bootstrap atinge diretamente sem rede de seguranca.

**Culebra e a rede de seguranca.**

Existe porque Mapanare precisava dela para sobreviver ao seu proprio bootstrap. Acontece que todo projeto de compilador que gera LLVM precisa da mesma coisa, mas ninguem havia empacotado isso antes.

Nao construimos apenas um linter. Construimos um motor de padroes. Cada bug de compilador que sobrevivemos se tornou um template para que ninguem mais precise sofrer com ele.

> O nome: *Mapanare* e uma vibora venezuelana. *Culebra* e a cobra comum. Mesma familia, papel diferente. Mapanare e a linguagem, Culebra e a ferramenta utilitaria que qualquer desenvolvedor de compiladores pode usar.

---

## Instalar

### Linux / macOS

```bash
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### Windows

```powershell
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### A partir do codigo fonte

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# Binario em target/release/culebra
```

Verificar:

```bash
culebra --version
```

---

## Inicio Rapido

Voce acabou de emitir `stage2.ll` do seu compilador e algo esta errado em tempo de execucao:

```bash
# 1. Escanear todos os padroes de bugs conhecidos
culebra scan stage2.ll

# 2. Apenas bugs criticos de ABI
culebra scan stage2.ll --tags abi --severity critical

# 3. Auto-corrigir o que for possivel
culebra scan stage2.ll --autofix --dry-run   # pre-visualizacao
culebra scan stage2.ll --autofix             # aplicar

# 4. O IR e valido?
culebra check stage2.ll

# 5. As constantes de string estao corretas?
culebra strings stage2.ll

# 6. Alguma patologia conhecida?
culebra audit stage2.ll

# 7. O que mudou entre stage1 e stage2?
culebra diff stage1.ll stage2.ll

# 8. Inspecionar uma funcao especifica
culebra extract stage2.ll minha_funcao_quebrada

# 9. Verificar layouts de structs contra o runtime C
culebra abi stage2.ll --header runtime/mapanare_core.c

# 10. Inspecionar o .rodata do binario compilado
culebra binary ./meu_compilador --ir stage2.ll --find "hello world"

# 11. Executar o pipeline completo
culebra pipeline
```

---

## Bugs reais que Culebra detecta

Bugs reais do bootstrap do Mapanare. Cada um desperdicou horas de depuracao.

### Constante de string sem alinhamento (o assassino do bootstrap)

Constantes de string sem `align 2` caem em enderecos impares. A marcacao de ponteiros desloca o ponteiro em -1 byte. Todas as comparacoes de string falham silenciosamente. O tokenizer produz 0 tokens. O compilador gera IR vazio. Sem crash, sem erro.

```bash
$ culebra scan stage2.ll --id unaligned-string-constant
CRITICAL [unaligned-string-constant] Constante de string sem alinhamento -- stage2.ll:47
  @.str.0 e uma constante de 6 bytes sem alinhamento.
  fix: Adicionar ', align 2' a todas as declaracoes [N x i8]
```

### Push em lista sem writeback (armadilha de analise de alias)

Push em uma lista via GEP diretamente em um campo de struct. LLVM cacheia o estado pre-push do struct. A mutacao e perdida. Stage 1 funciona, Stage 2 acumula 0 linhas.

```bash
$ culebra scan stage2.ll --id direct-push-no-writeback
HIGH [direct-push-no-writeback] Push em lista sem writeback -- stage2.ll:142 (em emit_line)
  Push em lista no campo 2 do struct vai diretamente por GEP sem
  temp alloca + writeback. LLVM pode otimizar a mutacao em -O1+.
```

---

## Motor de Templates

Culebra inclui um motor de padroes estilo Nuclei. Padroes de bugs sao templates YAML. O binario Rust e o motor. Os templates sao a base de conhecimento.

### Escanear

```bash
# Executar todos os templates
culebra scan file.ll

# Filtrar por tag, severidade ou template especifico
culebra scan file.ll --tags abi,string
culebra scan file.ll --severity critical,high
culebra scan file.ll --id unaligned-string-constant

# Verificacao cruzada de ABI
culebra scan file.ll --header runtime.c

# Auto-correcao
culebra scan file.ll --autofix --dry-run
culebra scan file.ll --autofix

# Formatos de saida
culebra scan file.ll --format json
culebra scan file.ll --format sarif     # GitHub Code Scanning
culebra scan file.ll --format markdown  # relatorios CI
```

### Explorar templates

```bash
culebra templates list
culebra templates list --tags abi
culebra templates show unaligned-string-constant
```

### Executar workflows

Workflows encadeiam templates com condicoes de parada:

```bash
culebra workflow bootstrap-health-check \
  --input stage1_output=stage1.ll

culebra workflow pre-commit \
  --input ir_file=main.ll
```

### Escreva seus proprios templates

Templates sao arquivos YAML em `culebra-templates/`:

```yaml
id: minha-verificacao
info:
  name: Minha verificacao personalizada
  severity: high
  author: seunome
  description: Detecta um padrao de bug especifico.
  tags:
    - ir
    - custom

scope:
  file_type: llvm-ir
  section: functions

match:
  matchers:
    - type: regex
      name: nome_padrao
      pattern:
        - 'algum padrao regex'
  condition: or

remediation:
  suggestion: "Como corrigir isso"
```

Qualquer pessoa construindo uma linguagem que gera LLVM pode contribuir seus proprios templates de bugs. O motor nao muda, a base de conhecimento cresce.

Veja [docs.md](../docs.md) para a especificacao completa.

---

## Templates Incluidos

17 templates em 4 categorias, todos de bugs reais do Mapanare.

| Categoria | ID | Severidade | O que detecta |
|---|---|---|---|
| **ABI** | `unaligned-string-constant` | Critica | Constantes de string em enderecos impares corrompem marcacao de ponteiros |
| **ABI** | `struct-layout-mismatch` | Critica | Divergencia de campos/tipos entre struct IR e header C |
| **ABI** | `direct-push-no-writeback` | Alta | Push em lista por GEP sem writeback via temp alloca |
| **ABI** | `sret-input-output-alias` | Alta | Ponteiro sret em alias com entrada corrompe dados |
| **ABI** | `tagged-pointer-odd-address` | Alta | Constantes de tamanho impar sem alinhamento quebram marcacao |
| **ABI** | `missing-byval-large-struct` | Media | Structs grandes passados como ptr sem byval |
| **IR** | `empty-switch-body` | Critica | Switch com 0 cases -- bracos de match nao gerados |
| **IR** | `ret-type-mismatch` | Critica | Tipo de retorno nao corresponde a assinatura da funcao |
| **IR** | `byte-count-mismatch` | Alta | Tamanho declarado `[N x i8]` vs conteudo real difere |
| **IR** | `phi-predecessor-mismatch` | Alta | No PHI referencia bloco predecessor inexistente |
| **IR** | `raw-control-byte-in-constant` | Media | Bytes de controle brutos em c"..." quebram ferramentas |
| **IR** | `unreachable-after-branch` | Media | Instrucoes apos terminador (codigo morto) |
| **Binario** | `missing-symbol` | Critica | Simbolo de runtime ausente na tabela de simbolos |
| **Binario** | `odd-address-rodata` | Alta | String em endereco impar na secao .rodata |
| **Bootstrap** | `function-count-drop` | Critica | Stage N+1 tem menos funcoes que Stage N |
| **Bootstrap** | `stage-output-divergence` | Alta | Saida do estagio nao converge para ponto fixo |
| **Bootstrap** | `fixed-point-delta` | Alta | Saida do compilador nao estabiliza apos N iteracoes |

---

## Todos os Comandos

| Comando | O que faz |
|---|---|
| `culebra scan file.ll` | Escanear IR com templates YAML. `--tags`, `--severity`, `--id`, `--format`, `--autofix`. |
| `culebra templates list` | Listar todos os templates disponiveis. |
| `culebra templates show <id>` | Mostrar detalhes completos de um template. |
| `culebra workflow <id>` | Executar um workflow de escaneamento multi-passo. |
| `culebra strings file.ll` | Validar contagens de bytes `[N x i8] c"..."`. |
| `culebra audit file.ll` | Detectar patologias de IR: switch vazio, incompatibilidade de ret, `%` ausente. |
| `culebra check file.ll` | Validar IR com `llvm-as`. |
| `culebra phi-check file.ll` | Validar que scripts de transformacao preservam a estrutura do IR. |
| `culebra diff a.ll b.ll` | Diff estrutural por funcao, normalizado por registradores. |
| `culebra extract file.ll fn` | Extrair uma funcao de um arquivo IR massivo. |
| `culebra table file.ll` | Tabela de metricas por funcao. |
| `culebra abi file.ll` | Detectar mau uso de sret/byref, validacao de layout de structs. |
| `culebra binary ./binario` | Inspecao ELF/PE, analise .rodata, referencia cruzada com IR. |
| `culebra run compilador fonte` | Compilar, executar, verificar saida esperada. |
| `culebra test` | Executar todos os `[[tests]]` de `culebra.toml`. |
| `culebra watch` | Observar arquivos, re-executar comando ao mudar. |
| `culebra pipeline` | Executar pipeline de estagios completo via `culebra.toml`. |
| `culebra fixedpoint compilador fonte` | Detectar convergencia de ponto fixo em compiladores auto-hospedados. |
| `culebra status` | Mostrar progresso de auto-hospedagem. |
| `culebra init` | Gerar template `culebra.toml`. |

---

## Configuracao: `culebra.toml`

Executar `culebra init` para gerar uma configuracao inicial:

```toml
[project]
name = "meu-compilador"
source_lang = "minha-linguagem"
target = "llvm"
compiler = "./meu-compilador"
runtime = "runtime/meu_runtime.c"

[[stages]]
name = "bootstrap"
cmd = "python bootstrap/compile.py {input}"
input = "src/compiler.ml"
output = "/tmp/stage1.ll"
validate = true

[[tests]]
name = "ola"
source = 'fn main() { print("ola") }'
expect = "ola"
```

---

## Arquitetura

```
                        culebra scan file.ll --tags abi
                                    |
                    +---------------+---------------+
                    |                               |
             Carregador de                     Parser de IR
              Templates                     (ir.rs -> IRModule)
          (culebra-templates/)
                    |                               |
                    +----------- Motor -------------+
                                    |
                    +---------------+---------------+
                    |               |               |
             Matcher Regex   Matcher Sequencia  Matcher Ref-Cruzada
            (linha unica)   (multi-linha com    (IR vs header C)
                             capturas, ausencia)
                    |               |               |
                    +--------- Descobertas ---------+
                                    |
                    +---------------+---------------+
                    |               |               |
                  Texto           JSON            SARIF
```

---

## Construido para

- Qualquer pessoa construindo uma linguagem que gera LLVM IR
- Qualquer pessoa auto-hospedando um compilador
- Qualquer pessoa depurando problemas de ABI e convencoes de chamada
- Qualquer pessoa executando um bootstrap multi-estagio
- Qualquer pessoa que queira converter seus bugs de compilador em templates de deteccao reutilizaveis

---

## Contribuir

Contribuicoes sao bem-vindas. Duas formas de contribuir:

1. **Codigo** -- Melhorias no motor Rust, novos tipos de matchers, formatos de saida
2. **Templates** -- Adicionar templates YAML para bugs de compilador que voce encontrou

---

## Licenca

Licenca MIT -- veja [LICENSE](../LICENSE) para detalhes.

---

<div align="center">

**Culebra** -- A rede de seguranca que seu compilador precisa.

[Documentacao Completa](../docs.md) · [Reportar Bug](https://github.com/Mapanare-Research/Culebra/issues) · [Mapanare](https://github.com/Mapanare-Research/Mapanare)

Feito com cuidado por [Juan Denis](https://juandenis.com)

</div>
