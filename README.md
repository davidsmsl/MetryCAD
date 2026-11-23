# MetryCAD

Versión afinada de [SolidCraft](https://github.com/tangency-labs/solidcraft) (v0.46.2, commit `bb08447c`): CAD isométrico y suite civil. Este repo es un fork completo; los cambios de MetryCAD son commits encima de ese commit (`git log`).

Código bajo MIT OR Apache-2.0 (ver `LICENSE-*`). El nombre y el logo de SolidCraft no se usan; ver `NOTICE-MetryCAD.md`.

## Compilar en Windows
En PowerShell 7 como administrador:

    Set-ExecutionPolicy -Scope Process Bypass -Force
    .\build\construir-metrycad.ps1

El script clona este repo y deja MSI, zip portable y `.exe` (`MetryCAD-*`) en `solidcraft\dist\release`.

## Estado
La fase 1 (grabador de macros, `crates/ui-egui/src/macro_recorder.rs`) aún no está compilada ni probada.
