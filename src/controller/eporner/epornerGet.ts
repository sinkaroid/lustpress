import { scrapeContent } from "../../scraper/eporner/epornerGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";

export async function getEporner(req: Request, res: Response) {
  try {
    const id = req.query.id as string;
    if (!id) throw Error("Parameter id is required");

    /**
     * @api {get} /eporner/get?id=:id Get eporner
     * @apiName Get eporner
     * @apiGroup eporner
     * @apiDescription Get a eporner video based on id
     *
     * @apiParam {String} id Video ID
     *
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     *
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/eporner/get?id=ibvqvezXzcs
     *
     * @apiExample {js} JS/TS
     * import axios from "axios"
     *
     * axios.get("https://lust.scathach.id/eporner/get?id=ibvqvezXzcs")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     *
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/eporner/get?id=ibvqvezXzcs") as resp:
     *    print(await resp.json())
     */
    // https://www.eporner.com/video-ibvqvezXzcs/ivy-seduces-her-dad-s-friend/
    // https://www.eporner.com/hd-porn/hgovoiPexQe/Risa-Tsukino/

    let path: string;

    if (id.startsWith("video-")) {
      path = id;
    } else {
      path = `hd-porn/${id}`;
    }

    const url = `${c.EPORNER}/${path}`;

    const data = await scrapeContent(url);
    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent"),
    });
    return res.json(data);
  } catch (err) {
    const e = err as Error;
    res.status(400).json(maybeError(false, e.message));
  }
}
