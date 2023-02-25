import React from "react";
import "./Dashboard.css";
import { useConfigResult } from "./ConfigProvider";
import BackendSettingsPage from "./BackendSettingsPage";
import ProgressPage from "./ProgressPage";

const Dashboard = () => {
    const configResult = useConfigResult();

    if (configResult.error) {
        return (
            <div>
                <div>{configResult.error}</div>
                <BackendSettingsPage />
            </div>
        );
    }
    if (configResult.config == null) {
        return <div>Loading... {configResult.progress}</div>;
    }
    return (
        <div>
            <div>
                <ProgressPage />
            </div>
        </div>
    );
};

export default Dashboard;
