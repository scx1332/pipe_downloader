import React, {useCallback, useContext, useState} from "react";
import {BackendSettingsContext} from "./BackendSettingsProvider";
import {useBackendConfig} from "./ConfigProvider";
import "./ProgressPage.css";
import {Simulate} from "react-dom/test-utils";
import load = Simulate.load;
import {backendFetch} from "./common/BackendCall";
import ProgressBar from 'react-bootstrap/ProgressBar';
import 'bootstrap/dist/css/bootstrap.min.css';


interface ProgressChunk {
    downloaded: number;
    to_download: number;
    to_unpack: number;
    unpacked: number;
}

interface Progress {
    chunkSize: number;
    chunksDownloading: number;
    chunksLeft: number;
    chunksTotal: number;
    currentChunks: { [key: number]: ProgressChunk }
    currentDownloadSpeed: number;
    currentUnpackSpeed: number;
    downloadUrl: string;
    downloaded: number;
    elapsedTimeSec: number;
    finished: boolean;
    errorMessage: string | null;
    errorMessageDownload: string | null;
    errorMessageUnpack: string | null;
    etaSec: number;
    finishTime: string;
    paused: boolean;
    startTime: string;
    stopRequested: boolean;
    totalDownloadSize: number;
    totalUnpackSize: number;
    unpacked: number;
}

interface ProgressChunkProps {
    chunk_no: number;
    chunk: ProgressChunk;
}
const ProgressChunk = (props: ProgressChunkProps) => {
    const chunk = props.chunk;

    const progressPercent = chunk.downloaded / chunk.to_download * 100;
    return <div className={"progress-chunk"}>
        <div>Chunk no {props.chunk_no}</div>
        <div>Downloaded: {chunk.downloaded}</div>
        <div>To download: {chunk.to_download}</div>
        <div>To unpack: {chunk.to_unpack}</div>
        <div>Unpacked: {chunk.unpacked}</div>
        <ProgressBar striped variant="success" now={progressPercent} />
    </div>
};

const ProgressPage = () => {
    const { backendSettings } = useContext(BackendSettingsContext);
    const config = useBackendConfig();
    const [progress, setProgress] = useState<Progress | null>(null);

    const [nextRefresh, setNextRefresh] = useState(0);

    const loadProgress = useCallback(async () => {
        try {
            const response = await backendFetch(backendSettings, `/progress`);
            const response_json = await response.json();
            setProgress(response_json.progress);
        } catch (e) {
            console.log(e);
            setProgress(null);
        }
    }, [setProgress]);

    React.useEffect(() => {
        console.log("Refreshing dashboard...");
        //timeout
        //sleep
        loadProgress().then(() => {
            setTimeout(() => {
                setNextRefresh(nextRefresh + 1);
            }, 2000);
        });
    }, [setNextRefresh, nextRefresh, loadProgress]);

    if (progress == null) {
        return (
            <div className="progress-page">
                <h1>Vite template</h1>
                <p>Connected to the endpoint {backendSettings.backendUrl}</p>
                <p>Frontend version {APP_VERSION}</p>
                <p>Backend version {config.version}</p>
                <p>Progress is null</p>
            </div>
        );
    }
    const progressPercent = progress.downloaded/progress.totalDownloadSize * 100;

    function row(key: number, chunk: ProgressChunk ) {
        return <ProgressChunk key={key} chunk_no={key} chunk={chunk} />;
    }

    return (
        <div className="progress-page">
            <h1>Vite template</h1>
            <p>Connected to the endpoint {backendSettings.backendUrl}</p>
            <p>Frontend version {APP_VERSION}</p>
            <p>Backend version {config.version}</p>

            <p>Downloading file: {progress.downloadUrl}</p>
            <p>Downloaded: {progress.downloaded}/{progress.totalDownloadSize}</p>
            <p>Unpacked: {progress.unpacked}</p>
            <ProgressBar striped variant="success" now={progressPercent} />
            {Object.entries(progress.currentChunks).map(([key, chunk]) => row(key, chunk))}
        </div>
    );
};

export default ProgressPage;