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
import prettyBytes from "pretty-bytes";
import DateBox from "./DateBox";

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
    finishTime: string | null;
    paused: boolean;
    startTime: string;
    currentTime: string;
    stopRequested: boolean;
    totalDownloadSize: number;
    totalUnpackSize: number;
    unpacked: number;
}

interface ProgressChunkProps {
    chunkWrapper: ProgressChunkWrapper;
}


const HumanBytes = (props: { bytes: number }) => {
    const bytesToHuman = (bytes: number) => {
        return prettyBytes(bytes, {minimumFractionDigits: 3})
    }
    return <span title={`${props.bytes} bytes`}>{bytesToHuman(props.bytes)}</span>
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
                    <div>Downloaded: <HumanBytes bytes={chunk.downloaded}/>/<HumanBytes bytes={chunk.toDownload}/></div>
                    <div>Unpack: <HumanBytes bytes={chunk.toUnpack}/>/<HumanBytes bytes={chunk.unpacked}/></div>
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
    const [finished, setFinished] = useState(false);

    const updateChunks = useCallback( (progress: Progress) => {
        for (const chunkNo in chunkInfos) {
            const chunkInfo = chunkInfos[chunkNo];
            if (chunkInfo.chunk.downloaded == chunkInfo.chunk.toDownload && chunkInfo.chunk.unpacked == chunkInfo.chunk.toUnpack) {
                if (chunkInfo.timeHidden && DateTime.now().diff(chunkInfo.timeHidden, 'seconds').seconds > 3) {
                    if (!(chunkNo in progress.currentChunks)) {
                        delete chunkInfos[chunkNo];
                        continue;
                    }
                }
                if (!chunkInfo.hidden && DateTime.now().diff(chunkInfo.timeShown, 'seconds').seconds > 2) {
                    chunkInfo.hidden = true;
                    chunkInfo.timeHidden = DateTime.now();
                }
            } else {
                if (chunkNo in progress.currentChunks) {
                    chunkInfo.chunk = progress.currentChunks[chunkNo];
                } else {
                    chunkInfo.chunk.downloaded = chunkInfo.chunk.toDownload;
                    chunkInfo.chunk.unpacked = chunkInfo.chunk.toUnpack;
                }
            }
        }
    }, [setProgress, chunkInfos, setChunkInfos, setFinished]);

    const loadProgress = useCallback(async () => {
        try {
            const response = await backendFetch(backendSettings, `/progress`);
            const response_json = await response.json();
            const progress : Progress = response_json.progress;
            setProgress(response_json.progress);

            updateChunks(response_json.progress);

            const finishTime = (progress.finishTime) ? DateTime.fromISO(progress.finishTime) : null;
            if (finishTime){
                setFinished(true);
            }

            for (const chunkNo in progress.currentChunks) {
                if (!(chunkNo in chunkInfos)) {
                    const chunk = progress.currentChunks[chunkNo];
                    const chunkNoInt = parseInt(chunkNo);
                    chunkInfos[chunkNo] = new ProgressChunkWrapper(chunk, chunkNoInt);
                }
            }
            setChunkInfos(chunkInfos);
        } catch (e) {
            console.log(e);
            setProgress(null);
        }
    }, [setProgress, chunkInfos, setChunkInfos, setFinished]);

    React.useEffect(() => {
        //timeout
        //sleep
        if (!finished) {
            loadProgress().then(() => {
                setTimeout(() => {
                    setNextRefresh(nextRefresh + 1);
                }, 1000);
            });
        } else {
            setTimeout(() => {
                setNextRefresh(nextRefresh + 1);
                if (progress) {
                    updateChunks(progress);
                }
            }, 1000);
        }
    }, [finished, setNextRefresh, nextRefresh, loadProgress]);


    function row(key: number, chunk: ProgressChunkWrapper ) {
        return <ProgressChunk key={key} chunkWrapper={chunk} />;
    }


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
    const isFinished = progress.finished;



    const progressPercent = progress.downloaded/progress.totalDownloadSize * 100;
    const serverTime = DateTime.fromISO(progress.currentTime);
    const etaSec = progress.etaSec;
    const eta = serverTime.plus({seconds: etaSec});
    const finishTime = (progress.finishTime) ? DateTime.fromISO(progress.finishTime) : null;
    const startTime = DateTime.fromISO(progress.startTime);

    return (
        <div className="progress-page">
            <div className="progress-page-left">
                <h4>Pipe downloader {config.version}</h4>
                <p>endpoint {backendSettings.backendUrl}</p>
                <div className={"progress-page-dates"}>
                    <DateBox date={progress.startTime} title={"Start time"} minimal={false}/>
                    <DateBox date={progress.currentTime} title={"Update time"} minimal={false}/>
                    {(progress.finishTime) ?  <DateBox date={progress.finishTime} title={"Finish time"} minimal={false}/>
                    :<DateBox date={eta.toISO()} title={"Estimated finish"} minimal={false}/>}
                </div>


                <div className={"download-url"} >
                    <div className={"header"}>Downloading file:</div>
                    <input readOnly={true} value={progress.downloadUrl}/>
                </div>

                {finishTime ? <div>
                    <table>
                        <tbody>
                        <tr>
                            <th>Successfully Downloaded:</th>
                            <td><HumanBytes bytes={progress.downloaded}/></td>
                        </tr>
                        <tr>
                            <th>Successfully unpacked:</th>
                            <td><HumanBytes bytes={progress.unpacked}/></td>
                        </tr>
                        <tr>
                            <th>Elapsed time:</th>
                            <td>{finishTime.diff(startTime).toFormat("hh:mm:ss")}</td>
                        </tr>
                        </tbody>
                    </table>
                </div> : <div>
                    <table>
                        <tbody>
                            <tr>
                                <th>Download speed:</th>
                                <td><HumanBytes bytes={progress.currentDownloadSpeed}/>/s</td>
                            </tr>
                            <tr>
                                <th>Unpack speed:</th>
                                <td><HumanBytes bytes={progress.currentUnpackSpeed}/>/s</td>
                            </tr>
                            <tr>
                                <th>Downloaded:</th>
                                <td><HumanBytes bytes={progress.downloaded}/>/<HumanBytes bytes={progress.totalDownloadSize}/></td>
                            </tr>
                            <tr>
                                <th>Unpacked:</th>
                                <td><HumanBytes bytes={progress.unpacked}/></td>
                            </tr>
                            <tr>
                                <th>Percent downloaded:</th>
                                <td>{progressPercent.toFixed(5)}%</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                }

                <ProgressBar striped variant="success" now={progressPercent} />
            </div>
            <div className="progress-page-right">
                <p>Chunks left: {progress.chunksLeft} finished: {progress.chunksTotal - progress.chunksLeft} Active: {Object.keys(chunkInfos).length}</p>
                {Object.entries(chunkInfos).reverse().map(([key, chunk]) => row(parseInt(key), chunk))}
            </div>
        </div>
    );
};

export default ProgressPage;