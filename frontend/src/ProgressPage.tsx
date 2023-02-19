import React, {useCallback, useContext, useState} from "react";
import {BackendSettingsContext} from "./BackendSettingsProvider";
import {useBackendConfig} from "./ConfigProvider";
import "./ProgressPage.css";
import {Simulate} from "react-dom/test-utils";
import load = Simulate.load;
import {backendFetch} from "./common/BackendCall";
import ProgressBar from 'react-bootstrap/ProgressBar';
import 'bootstrap/dist/css/bootstrap.min.css';
import Collapse from 'react-bootstrap/Collapse';
import Fade from 'react-bootstrap/Fade';
import {DateTime} from "luxon";

interface ProgressChunk {
    downloaded: number;
    toDownload: number;
    toUnpack: number;
    unpacked: number;
}

class ProgressChunkWrapper {
    chunk: ProgressChunk;
    chunkNo: number;

    hidden: boolean;

    timeShown: DateTime;

    timeHidden: DateTime | null;

    constructor(chunk: ProgressChunk, chunkNo: number) {
        this.chunk = chunk;
        this.chunkNo = chunkNo;
        this.hidden = false;
        this.timeShown = DateTime.now();
        this.timeHidden = null;
    }
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
    chunkWrapper: ProgressChunkWrapper;
}


const ProgressChunk = (props: ProgressChunkProps) => {
    const chunkNo = props.chunkWrapper.chunkNo;
    const chunk = props.chunkWrapper.chunk;

    const [visible, setVisible] = useState(false);

    React.useEffect(() => {
        setTimeout(() => {
            setVisible(!props.chunkWrapper.hidden);
        }, 100);
    }, [props.chunkWrapper.hidden]);

    const progressPercent = chunk.downloaded / chunk.toDownload * 100;

    let className = "progress-chunk";
    if (visible) {
        className += " progress-chunk-visible";
    } else {
        className += " progress-chunk-hidden";
    }

    return <div className={className}>
            <div className={"progress-chunk-inner"}>
                <div className={"progress-chunk-header"}>
                    <div>Chunk no {chunkNo}</div>
                    <div>Downloaded: {chunk.downloaded}</div>
                    <div>To download: {chunk.toDownload}</div>
                    <div>To unpack: {chunk.toUnpack}</div>
                    <div>Unpacked: {chunk.unpacked}</div>
                </div>
                <ProgressBar striped variant="success" now={progressPercent} />
            </div>
        </div>
};

const ProgressPage = () => {
    const { backendSettings } = useContext(BackendSettingsContext);
    const config = useBackendConfig();
    const [progress, setProgress] = useState<Progress | null>(null);

    const [nextRefresh, setNextRefresh] = useState(0);
    const [chunkInfos, setChunkInfos] = useState<{ [key: number]: ProgressChunkWrapper }>({})

    const loadProgress = useCallback(async () => {
        try {
            const response = await backendFetch(backendSettings, `/progress`);
            const response_json = await response.json();
            const progress : Progress = response_json.progress;
            setProgress(response_json.progress);

            for (const chunkNo in chunkInfos) {
                const chunkInfo = chunkInfos[chunkNo];
                if (chunkInfo.chunk.downloaded == chunkInfo.chunk.toDownload && chunkInfo.chunk.unpacked == chunkInfo.chunk.toUnpack) {
                    if (chunkInfo.timeHidden && DateTime.now().diff(chunkInfo.timeHidden, 'seconds').seconds > 3) {
                        if (!(chunkNo in progress.currentChunks)) {
                            delete chunkInfos[chunkNo];
                            continue;
                        }
                    }
                    if (!chunkInfo.hidden && DateTime.now().diff(chunkInfo.timeShown, 'seconds').seconds > 3) {
                        chunkInfo.hidden = true;
                        chunkInfo.timeHidden = DateTime.now();
                    }
                } else {
                    if (chunkNo in progress.currentChunks) {
                        chunkInfo.chunk = progress.currentChunks[chunkNo];
                    }
                }
            }

            for (const chunkNo in progress.currentChunks) {
                if (!(chunkNo in chunkInfos)) {
                    const chunk = progress.currentChunks[chunkNo];
                    chunkInfos[chunkNo] = new ProgressChunkWrapper(chunk, chunkNo);
                }
            }
            setChunkInfos(chunkInfos);
        } catch (e) {
            console.log(e);
            setProgress(null);
        }
    }, [setProgress, chunkInfos, setChunkInfos]);

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

    function row(key: number, chunk: ProgressChunkWrapper ) {
        return <ProgressChunk key={key} chunkWrapper={chunk} />;
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
            {Object.entries(chunkInfos).reverse().map(([key, chunk]) => row(key, chunk))}
        </div>
    );
};

export default ProgressPage;