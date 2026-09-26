# tty-office formula reference

Formulas evaluate in Excel dialect on a single-sheet mirror, backed by
the `formualizer` engine. Package formulas arriving in ODS form
(`of:=[.A1]+1`) convert to canonical form on open and back on save;
the list separator converts between `;` and `,`, while function
names pass through unchanged.

## Tested functions

Every name below is pinned by `tests/sheet_formulas.rs`; the suite
fails if engine upgrades change their values.

| Family | Functions |
|--------|-----------|
| Aggregates | SUM, AVERAGE, MIN, MAX, COUNT over ranges and lists |
| Operators | `+ - * /`, comparisons returning TRUE/FALSE |
| Logical | IF, AND, OR |
| Text | UPPER, LEN, LEFT, CONCAT |
| Math | ABS, SQRT, POWER, MOD |

## Wider engine coverage

The engine ships further families which the battery does not pin:
trigonometry, date and time, financial, engineering, lookup
(including XLOOKUP), and statistics. Names outside the engine
surface as error text in the cell rather than failing the open.

## Failure semantics

Unknown functions, division by zero, and circular references all
terminate as cell error values; none of them hangs or panics. The
circular-reference test exists precisely to prove termination.

## Recalculation scope

Each edit re-evaluates only the formulas that transitively depend
on the touched cells, found by scanning references out of the
formula text. Anything the scanner cannot analyze (structured
references, defined names, whole-column ranges) falls back to a
full pass, so doubt costs time but never correctness through a
missed edge. Typing on a formula-heavy sheet therefore stays
responsive instead of re-running the whole workbook per keystroke.

## Limits

References resolve within the open sheet only. Cross-sheet
references (`Sheet2!A1`) do not evaluate until workbook support
lands; they surface as errors rather than stale values.
