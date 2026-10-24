# ODK v1 provides the UI layer, Omarchy theming and the Plugin machinery only

ODK v1 is the UI layer over GPUI (ADR 0004), Omarchy theme integration (ADR 0019) and the Luau Plugin machinery (ADR 0003, ADR 0020). Application scaffolding such as settings storage, single-instance handling and app-to-app IPC is left to each application until a real second application shows a shared need, so ODK does not guess at an abstraction too early.
