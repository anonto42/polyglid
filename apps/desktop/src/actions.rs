use gpui::actions;

actions!(
    polyglid,
    [
        OpenCommandPalette,
        CloseOverlay,
        ToggleSidebar,
        ToggleBottomPanel,
        OpenProjects,
        OpenScanner,
        OpenExecutions,
        OpenReports,
        OpenPlugins,
        CloseActiveTab,
        NextTab,
        SaveDocument,
        RefreshWorkspace,
        Quit
    ]
);
