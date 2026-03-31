<div align="center">

# Culebra

**/koo-LEH-brah/**

**Diagnosticos de compilador para lenguajes auto-hospedados que generan LLVM.**

*ABI. IR. Binario. Bootstrap. Un solo binario detecta lo que ningun depurador puede.*

Nacio del bootstrap de [Mapanare](https://github.com/Mapanare-Research/Mapanare), donde cada bug era un misterio sin red de seguridad. Culebra incluye un motor de plantillas estilo Nuclei para que cada bug de compilador que sobrevivas se convierta en un patron que nadie mas tenga que depurar.

[English](../README.md) | Espanol | [中文版](README.zh-CN.md) | [Portugues](README.pt.md)

<br>

![Rust](https://img.shields.io/badge/Rust-Edicion_2021-dea584?style=for-the-badge&logo=rust&logoColor=white)
![LLVM](https://img.shields.io/badge/LLVM-Analisis_IR-262D3A?style=for-the-badge&logo=llvm&logoColor=white)
![Plataforma](https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-grey?style=for-the-badge)

[![Licencia](https://img.shields.io/badge/licencia-MIT-green.svg?style=flat-square)](../LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg?style=flat-square)](../Cargo.toml)
[![Plantillas](https://img.shields.io/badge/plantillas-17_incluidas-orange.svg?style=flat-square)]()
[![GitHub Stars](https://img.shields.io/github/stars/Mapanare-Research/Culebra?style=flat-square&color=f5c542)](https://github.com/Mapanare-Research/Culebra/stargazers)

<br>

[Por que Culebra?](#por-que-culebra) · [Instalar](#instalar) · [Inicio Rapido](#inicio-rapido) · [Motor de Plantillas](#motor-de-plantillas) · [Todos los Comandos](#todos-los-comandos) · [Plantillas Incluidas](#plantillas-incluidas) · [Configuracion](#configuracion-culebratoml) · [Arquitectura](#arquitectura) · [Documentacion](../docs.md) · [Contribuir](#contribuir)

</div>

---

## Por que Culebra?

La mayoria de los lenguajes se auto-hospedaron sobre un compilador maduro:

- **Rust** comenzo en OCaml antes de auto-hospedarse un ano despues.
- **Go** fue escrito en C hasta v1.5, luego uso un traductor automatico de C a Go.
- **C++** se auto-hospedo a traves de Cfront, que traducia C++ a C.

[Mapanare](https://github.com/Mapanare-Research/Mapanare) no tiene ese lujo. Es un lenguaje compilado AI-nativo que genera LLVM IR, construyendo su propio backend desde cero: lexer, AST, inferencia de tipos, emision de LLVM IR. El compilador bootstrap (Stage 0) esta escrito en Python, pero no hay un compilador maduro debajo como respaldo.

Eso significa que cada desajuste de ABI, cada error de conteo de bytes en strings, cada divergencia de layout de structs entre IR y C, cada regresion de etapa de bootstrap golpea directamente sin red de seguridad.

**Culebra es la red de seguridad.**

Existe porque Mapanare lo necesitaba para sobrevivir su propio bootstrap. Resulta que todo proyecto de compilador que genera LLVM necesita lo mismo, pero nadie lo habia empaquetado antes.

No solo construimos un linter. Construimos un motor de patrones. Cada bug de compilador que sobrevivimos se convirtio en una plantilla para que nadie mas tenga que sufrirlo.

> El nombre: *Mapanare* es una vibora venezolana. *Culebra* es la serpiente comun. Misma familia, diferente rol. Mapanare es el lenguaje, Culebra es la herramienta que cualquier desarrollador de compiladores puede usar.

*Hecho con orgullo venezolano* 

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

### Desde el codigo fuente

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# Binario en target/release/culebra
```

Verificar:

```bash
culebra --version
```

---

## Inicio Rapido

Acabas de emitir `stage2.ll` desde tu compilador y algo falla en tiempo de ejecucion:

```bash
# 1. Escanear todos los patrones de bugs conocidos
culebra scan stage2.ll

# 2. Solo bugs criticos de ABI
culebra scan stage2.ll --tags abi --severity critical

# 3. Auto-corregir lo que se pueda
culebra scan stage2.ll --autofix --dry-run   # vista previa
culebra scan stage2.ll --autofix             # aplicar

# 4. Es valido el IR?
culebra check stage2.ll

# 5. Estan correctas las constantes de string?
culebra strings stage2.ll

# 6. Alguna patologia conocida?
culebra audit stage2.ll

# 7. Que cambio entre stage1 y stage2?
culebra diff stage1.ll stage2.ll

# 8. Inspeccionar una funcion especifica
culebra extract stage2.ll mi_funcion_rota

# 9. Verificar layouts de structs contra el runtime C
culebra abi stage2.ll --header runtime/mapanare_core.c

# 10. Inspeccionar el .rodata del binario compilado
culebra binary ./mi_compilador --ir stage2.ll --find "hello world"

# 11. Ejecutar el pipeline completo
culebra pipeline
```

---

## Bugs reales que Culebra detecta

Bugs reales del bootstrap de Mapanare. Cada uno desperdicio horas de depuracion.

### Constante de string sin alineacion (el asesino del bootstrap)

Constantes de string sin `align 2` caen en direcciones impares. El etiquetado de punteros desplaza el puntero -1 byte. Todas las comparaciones de string fallan silenciosamente. El tokenizer produce 0 tokens. El compilador genera IR vacio. Sin crash, sin error.

```bash
$ culebra scan stage2.ll --id unaligned-string-constant
CRITICAL [unaligned-string-constant] Constante de string sin alineacion -- stage2.ll:47
  @.str.0 es una constante de 6 bytes sin alineacion.
  fix: Agregar ', align 2' a todas las declaraciones [N x i8]
```

### Desajuste de conteo de bytes en string

Tu manejador de secuencias de escape emite `\n` como dos bytes en lugar de uno pero el tipo `[N x i8]` dice `N`. El IR se ensambla, el binario enlaza, y el string silenciosamente contiene basura.

```bash
$ culebra strings stage2.ll
ERROR: @.str.47 declara [14 x i8] pero el contenido tiene 13 bytes
  -> c"Hello, world!\00"
  Fix: cambiar a [13 x i8]
```

### Push a lista sin writeback (trampa de analisis de alias)

Push a una lista via GEP directamente en un campo de struct. LLVM cachea el estado pre-push del struct. La mutacion se pierde. Stage 1 funciona, stage 2 acumula 0 lineas.

```bash
$ culebra scan stage2.ll --id direct-push-no-writeback
HIGH [direct-push-no-writeback] Push a lista sin writeback -- stage2.ll:142 (en emit_line)
  Push a lista en campo 2 del struct va directamente por GEP sin
  temp alloca + writeback. LLVM puede optimizar la mutacion en -O1+.
```

---

## Motor de Plantillas

Culebra incluye un motor de patrones estilo Nuclei. Los patrones de bugs son plantillas YAML. El binario Rust es el motor. Las plantillas son la base de conocimiento.

### Escanear

```bash
# Ejecutar todas las plantillas
culebra scan file.ll

# Filtrar por etiqueta, severidad, o plantilla especifica
culebra scan file.ll --tags abi,string
culebra scan file.ll --severity critical,high
culebra scan file.ll --id unaligned-string-constant

# Verificacion cruzada de ABI
culebra scan file.ll --header runtime.c

# Auto-correccion
culebra scan file.ll --autofix --dry-run
culebra scan file.ll --autofix

# Formatos de salida
culebra scan file.ll --format json
culebra scan file.ll --format sarif     # GitHub Code Scanning
culebra scan file.ll --format markdown  # reportes CI
```

### Explorar plantillas

```bash
culebra templates list
culebra templates list --tags abi
culebra templates show unaligned-string-constant
```

### Ejecutar workflows

Los workflows encadenan plantillas con condiciones de parada:

```bash
culebra workflow bootstrap-health-check \
  --input stage1_output=stage1.ll

culebra workflow pre-commit \
  --input ir_file=main.ll
```

### Escribe tus propias plantillas

Las plantillas son archivos YAML en `culebra-templates/`:

```yaml
id: mi-verificacion
info:
  name: Mi verificacion personalizada
  severity: high
  author: tunombre
  description: Detecta un patron de bug especifico.
  tags:
    - ir
    - custom

scope:
  file_type: llvm-ir
  section: functions

match:
  matchers:
    - type: regex
      name: nombre_patron
      pattern:
        - 'algun patron regex'
  condition: or

remediation:
  suggestion: "Como corregir esto"
```

Cualquiera construyendo un lenguaje que genera LLVM puede contribuir sus propias plantillas de bugs. El motor no cambia, la base de conocimiento crece.

Ver [docs.md](../docs.md) para la especificacion completa.

---

## Plantillas Incluidas

17 plantillas en 4 categorias, todas de bugs reales de Mapanare.

| Categoria | ID | Severidad | Que detecta |
|---|---|---|---|
| **ABI** | `unaligned-string-constant` | Critica | Constantes de string en direcciones impares corrompen etiquetado de punteros |
| **ABI** | `struct-layout-mismatch` | Critica | Divergencia de campos/tipos entre struct IR y header C |
| **ABI** | `direct-push-no-writeback` | Alta | Push a lista por GEP sin writeback via temp alloca |
| **ABI** | `sret-input-output-alias` | Alta | Puntero sret aliasing con entrada corrompe datos |
| **ABI** | `tagged-pointer-odd-address` | Alta | Constantes de tamano impar sin alineacion rompen etiquetado |
| **ABI** | `missing-byval-large-struct` | Media | Structs grandes pasados como ptr sin byval |
| **IR** | `empty-switch-body` | Critica | Switch con 0 casos -- brazos de match no generados |
| **IR** | `ret-type-mismatch` | Critica | Tipo de retorno no coincide con firma de funcion |
| **IR** | `byte-count-mismatch` | Alta | Tamano declarado `[N x i8]` vs contenido real difiere |
| **IR** | `phi-predecessor-mismatch` | Alta | Nodo PHI referencia bloque predecesor inexistente |
| **IR** | `raw-control-byte-in-constant` | Media | Bytes de control crudos en c"..." rompen herramientas |
| **IR** | `unreachable-after-branch` | Media | Instrucciones despues de terminador (codigo muerto) |
| **Binario** | `missing-symbol` | Critica | Simbolo de runtime faltante en tabla de simbolos |
| **Binario** | `odd-address-rodata` | Alta | String en direccion impar en seccion .rodata |
| **Bootstrap** | `function-count-drop` | Critica | Stage N+1 tiene menos funciones que Stage N |
| **Bootstrap** | `stage-output-divergence` | Alta | Salida de stage no converge hacia punto fijo |
| **Bootstrap** | `fixed-point-delta` | Alta | Salida del compilador no se estabiliza despues de N iteraciones |

---

## Todos los Comandos

| Comando | Que hace |
|---|---|
| `culebra scan file.ll` | Escanear IR con plantillas YAML. `--tags`, `--severity`, `--id`, `--format`, `--autofix`. |
| `culebra templates list` | Listar todas las plantillas disponibles. |
| `culebra templates show <id>` | Mostrar detalles completos de una plantilla. |
| `culebra workflow <id>` | Ejecutar un workflow de escaneo multi-paso. |
| `culebra strings file.ll` | Validar conteos de bytes `[N x i8] c"..."`. |
| `culebra audit file.ll` | Detectar patologias de IR: switch vacio, desajuste de ret, `%` faltante. |
| `culebra check file.ll` | Validar IR con `llvm-as`. |
| `culebra phi-check file.ll` | Validar que scripts de transformacion preservan la estructura del IR. |
| `culebra diff a.ll b.ll` | Diff estructural por funcion, normalizado por registros. |
| `culebra extract file.ll fn` | Extraer una funcion de un archivo IR masivo. |
| `culebra table file.ll` | Tabla de metricas por funcion. |
| `culebra abi file.ll` | Detectar mal uso de sret/byref, validacion de layout de structs. |
| `culebra binary ./binario` | Inspeccion ELF/PE, analisis .rodata, referencia cruzada con IR. |
| `culebra run compilador fuente` | Compilar, ejecutar, verificar salida esperada. |
| `culebra test` | Ejecutar todos los `[[tests]]` de `culebra.toml`. |
| `culebra watch` | Observar archivos, re-ejecutar comando al cambiar. |
| `culebra pipeline` | Ejecutar pipeline de stages completo via `culebra.toml`. |
| `culebra fixedpoint compilador fuente` | Detectar convergencia de punto fijo en compiladores auto-hospedados. |
| `culebra status` | Mostrar progreso de auto-hospedaje. |
| `culebra init` | Generar plantilla `culebra.toml`. |

---

## Configuracion: `culebra.toml`

Ejecutar `culebra init` para generar una configuracion inicial:

```toml
[project]
name = "mi-compilador"
source_lang = "mi-lenguaje"
target = "llvm"
compiler = "./mi-compilador"
runtime = "runtime/mi_runtime.c"

[[stages]]
name = "bootstrap"
cmd = "python bootstrap/compile.py {input}"
input = "src/compiler.ml"
output = "/tmp/stage1.ll"
validate = true

[[tests]]
name = "hola"
source = 'fn main() { print("hola") }'
expect = "hola"
```

---

## Arquitectura

```
                        culebra scan file.ll --tags abi
                                    |
                    +---------------+---------------+
                    |                               |
             Cargador de                       Parser de IR
              Plantillas                    (ir.rs -> IRModule)
          (culebra-templates/)
                    |                               |
                    +----------- Motor -------------+
                                    |
                    +---------------+---------------+
                    |               |               |
              Matcher Regex   Matcher Secuencia  Matcher Ref-Cruzada
             (linea unica)   (multi-linea con    (IR vs header C)
                             capturas, ausencia)
                    |               |               |
                    +---------- Hallazgos ----------+
                                    |
                    +---------------+---------------+
                    |               |               |
                  Texto           JSON            SARIF
```

---

## Construido para

- Cualquiera construyendo un lenguaje que genera LLVM IR
- Cualquiera auto-hospedando un compilador
- Cualquiera depurando problemas de ABI y convenciones de llamada
- Cualquiera ejecutando un bootstrap multi-stage
- Cualquiera que quiera convertir sus bugs de compilador en plantillas de deteccion reutilizables

---

## Contribuir

Contribuciones bienvenidas. Dos formas de contribuir:

1. **Codigo** -- Mejoras al motor Rust, nuevos tipos de matchers, formatos de salida
2. **Plantillas** -- Agregar plantillas YAML para bugs de compilador que hayas encontrado

---

## Licencia

Licencia MIT -- ver [LICENSE](../LICENSE) para detalles.

---

<div align="center">

**Culebra** -- La red de seguridad que tu compilador necesita.

[Documentacion Completa](../docs.md) · [Reportar Bug](https://github.com/Mapanare-Research/Culebra/issues) · [Mapanare](https://github.com/Mapanare-Research/Mapanare)

Hecho con cuidado por [Juan Denis](https://juandenis.com)

</div>
