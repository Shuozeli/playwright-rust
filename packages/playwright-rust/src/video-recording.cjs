'use strict';

module.exports = ({ UnsupportedOperationError, mapError, resolvePath, ffmpegPath }) => {
  function prepare(recordVideo, viewport, api, videosPath, videoSize) {
    if (videoSize !== undefined && videosPath === undefined)
      throw new Error('"videoSize" option requires "videosPath" to be specified');
    if (videosPath !== undefined && typeof videosPath !== 'string')
      throw new TypeError(`${api}: videosPath must be a string`);
    if (recordVideo === undefined && videosPath !== undefined)
      recordVideo = { dir: videosPath, size: videoSize };
    if (recordVideo === undefined) return undefined;
    if (recordVideo === null || typeof recordVideo !== 'object' || Array.isArray(recordVideo))
      throw new TypeError(`${api}: recordVideo must be an object`);

    recordVideo = { ...recordVideo };
    for (const key of Object.keys(recordVideo)) {
      if (!['dir', 'size'].includes(key))
        throw new TypeError(`${api}: recordVideo.${key} is not supported`);
    }
    if (typeof recordVideo.dir !== 'string')
      throw new TypeError(`recordVideo.dir: expected string, got ${typeof recordVideo.dir}`);

    let width;
    let height;
    if (recordVideo.size === undefined) {
      if (viewport === null) {
        width = 800;
        height = 600;
      } else {
        const source = viewport && typeof viewport === 'object'
          ? viewport
          : { width: 1280, height: 720 };
        const scale = Math.min(1, 800 / Math.max(source.width, source.height));
        width = Math.floor(source.width * scale);
        height = Math.floor(source.height * scale);
      }
    } else {
      const size = recordVideo.size;
      if (size === null || typeof size !== 'object' || Array.isArray(size))
        throw new TypeError(`${api}: recordVideo.size must be an object`);
      const copy = { ...size };
      for (const key of Object.keys(copy)) {
        if (!['width', 'height'].includes(key))
          throw new TypeError(`${api}: recordVideo.size.${key} is not supported`);
      }
      width = copy.width;
      height = copy.height;
    }
    for (const [name, value] of [['width', width], ['height', height]]) {
      if (!Number.isInteger(value) || value < 2 || value > 0xffff_ffff)
        throw new TypeError(`${api}: recordVideo.size.${name} must be an integer in [2, 4294967295]`);
    }

    return Object.freeze({
      directory: resolvePath(recordVideo.dir),
      ffmpeg: ffmpegPath(),
      width,
      height,
    });
  }

  async function initialize(session, contextId, plan) {
    if (plan === undefined) return;
    if (typeof session.installContextVideo !== 'function')
      throw new UnsupportedOperationError('browser.newContext recordVideo (select video-recording native profile)');
    await session.installContextVideo(
      contextId,
      plan.directory,
      plan.ffmpeg,
      plan.width,
      plan.height,
    );
  }

  async function newBrowserPage(session, baseURL, acceptDownloads, plan) {
    if (typeof session.newBrowserPageWithVideo !== 'function')
      throw new UnsupportedOperationError('browser.newPage recordVideo (select video-recording native profile)');
    return await session.newBrowserPageWithVideo(
      baseURL,
      acceptDownloads,
      plan.directory,
      plan.ffmpeg,
      plan.width,
      plan.height,
    );
  }

  class Video {
    constructor(page) {
      this._page = page;
    }

    async path() {
      const session = this._page._context._browser._session;
      if (typeof session.pageVideoPath !== 'function')
        throw new UnsupportedOperationError('video.path (select video-recording native profile)');
      try { return await session.pageVideoPath(this._page._id); }
      catch (error) { throw mapError(error, 'video.path'); }
    }

    async saveAs(path) {
      if (typeof path !== 'string') throw new TypeError('video.saveAs: path must be a string');
      const session = this._page._context._browser._session;
      if (typeof session.pageVideoSaveAs !== 'function')
        throw new UnsupportedOperationError('video.saveAs (select video-recording native profile)');
      try { await session.pageVideoSaveAs(this._page._id, resolvePath(path)); }
      catch (error) { throw mapError(error, 'video.saveAs'); }
    }

    async delete() {
      const session = this._page._context._browser._session;
      if (typeof session.pageVideoDelete !== 'function')
        throw new UnsupportedOperationError('video.delete (select video-recording native profile)');
      try { await session.pageVideoDelete(this._page._id); }
      catch (error) { throw mapError(error, 'video.delete'); }
    }
  }

  return {
    create: page => new Video(page),
    initialize,
    newBrowserPage,
    prepare,
  };
};
