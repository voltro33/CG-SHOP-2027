# Golden fixtures

These scripts ran the Python solver and wrote down what it produced, so each
Rust crate can prove it matches. Their output is checked in under each crate's
`tests/fixtures/`.

| Script                         | Writes                                                  |
| ------------------------------ | ------------------------------------------------------- |
| `io/export_problems.py`        | `crates/adapters/io/tests/fixtures/`                    |
| `io/load_time.py`              | nothing, prints Python's load time and memory           |
| `coverage/make_fixtures.py`    | `crates/domain/coverage/tests/fixtures/`                |
| `order/export_fixtures.py`     | `crates/plugins/order/tests/fixtures/`                  |
| `decompose/export_fixtures.py` | `crates/plugins/decompose/tests/fixtures/`              |
| `improve/export_fixtures.py`   | `crates/plugins/improve/tests/fixtures/pipeline/`       |
| `improve/export_damaged.py`    | `crates/plugins/improve/tests/fixtures/damaged/`        |
| `decompose/export_mixed.py`    | `crates/plugins/decompose/tests/fixtures/mixed/`        |
| `assign/export_fixtures.py`    | `crates/plugins/assign/tests/fixtures/`                 |
| `cli/export_fixtures.py`       | `crates/app/cli/tests/fixtures/` (job grid, lane snake) |

They import the Python `solver` package, which this branch no longer has. To run
one again, check out `init-solver` in a worktree, copy the script there and run
it from that worktree's root:

```bash
git worktree add ../cgshop-python init-solver
cd ../cgshop-python
python <path to the script>    # with the package and ortools installed
```

Then copy the fixture it wrote back into the crate.
