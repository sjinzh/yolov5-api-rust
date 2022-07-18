use opencv::{
    core::{self, MatTraitConst, MatTrait, MatExprTraitConst},
    dnn::{self, NetTraitConst, NetTrait}
};
use std::{fs::File, io::{BufReader}, error::Error};

use serde::{Deserialize};
pub struct PadInfo {

    pub mat: core::Mat,
    pub top: i32,
    pub left: i32

}

pub struct DetectionOuput {

    pub boxes: core::Vector<opencv::core::Rect>,
    pub scores: core::Vector<f32>,

    pub indices: core::Vector<i32>,

    pub class_index_list: Vec<i32>
}

#[derive(Deserialize)]
pub struct ModelConfig {

    pub model_path : String,
    pub class_names : Vec<String>,
    pub input_size: i32
}

pub struct Model {

    pub model: dnn::Net,
    pub model_config: ModelConfig
}

pub fn load_model_from_config() -> Result<ModelConfig, Box<dyn Error>>{

    let file = File::open("data/config.json")?;
    let reader = BufReader::new(file);

    let j : ModelConfig = serde_json::from_reader(reader)?;

    println!("{model_path}", model_path=j.model_path);

    Ok(j)
}


pub fn detect(model_data: &mut Model, img: &core::Mat) -> opencv::Result<()> {
    
    let model = &mut model_data.model;

    let model_config = &mut model_data.model_config;
   
    let mat_copy = img.clone();

    // letterbox

    let pad_info = letterbox(&mat_copy, core::Size::new(model_config.input_size, model_config.input_size), true)?;

    let padded_mat = pad_info.mat.clone();

    // dnn blob

    let blob = opencv::dnn::blob_from_image(&padded_mat, 1.0 / 255.0, opencv::core::Size_{width: model_config.input_size, height: model_config.input_size}, core::Scalar::new(0f64,0f64,0f64,0f64), true, false, core::CV_32F)?;

    let out_layer_names = model.get_unconnected_out_layers_names()?;

    let mut outs : opencv::core::Vector<core::Mat> = opencv::core::Vector::default();
    model.set_input(&blob, "", 1.0, core::Scalar::default())?;
    
    model.forward(&mut outs, &out_layer_names)?;

    let detection_output = post_process(&outs,0.5, 0.5)?;

    draw_predictions(&mut pad_info.mat.clone(), &detection_output)?;
    
    
    Ok(())
}

fn letterbox( img: &core::Mat, new_shape: core::Size, scale_up: bool) -> opencv::Result<PadInfo> {


    let width = img.cols() as f32;
    let height = img.rows() as f32;

    let new_width = new_shape.width as f32;
    let new_height = new_shape.height as f32;
    let mut r = f32::min(new_width / width, new_height / height );

    if !scale_up {
        r =f32::min(r, 1.0);
    }

    let new_unpad_w = (width * r).round() as i32;
    let new_unpad_h = (height * r).round() as i32;

    let dw = (new_shape.width - new_unpad_w) / 2;
    let dh = (new_shape.height - new_unpad_h) / 2;

    let mut dst = core::Mat::default();
    opencv::imgproc::resize(&img, &mut dst, core::Size_{width: new_unpad_w, height: new_unpad_h}, 0.0, 0.0, opencv::imgproc::INTER_LINEAR)?;

    let top =  (dh as f32 - 0.1).round() as i32;
    let bottom =  (dh as f32 + 0.1).round() as i32;
    let left =  (dw as f32 - 0.1).round() as i32;
    let right =  (dw as f32 + 0.1).round() as i32;

    let mut final_mat = core::Mat::default();
    opencv::core::copy_make_border(&dst, &mut final_mat, top, bottom, left, right, opencv::core::BORDER_CONSTANT, opencv::core::Scalar::new(114.0, 114.0, 114.0, 114.0))?;
    
    //let params: core::Vector<i32> = core::Vector::default();
    
    //opencv::imgcodecs::imwrite("padded.jpg", &final_mat, &params)?;
    
    Ok(PadInfo{mat: final_mat, top: top, left: left})
}


use std::os::raw::c_void;


fn post_process(outs: &core::Vector<core::Mat>, conf_thresh: f32, nms_thresh: f32 ) -> opencv::Result<DetectionOuput>{

    
    let mut det = outs.get(0)?;

    let rows = *det.mat_size().get(1).unwrap();
    let cols = *det.mat_size().get(2).unwrap();
    
    let mut boxes: core::Vector<opencv::core::Rect> = core::Vector::new();
    let mut scores: core::Vector<f32> = core::Vector::new();

    let mut indices: core::Vector<i32> = core::Vector::new();

    let mut class_index_list: core::Vector<i32> = core::Vector::new();

    unsafe {
      
        let data = det.ptr_mut(0)?.cast::<c_void>();

        let m = core::Mat::new_rows_cols_with_data(rows, cols, core::CV_32F, data, core::Mat_AUTO_STEP )?; // std::mem::size_of::<f32>()* 41
        
        for r in 0..m.rows() {

            //println!("begin");
            let cx: &f32 = m.at_2d::<f32>(r, 0)?;
            let cy: &f32 = m.at_2d::<f32>(r, 1)?;
            let w: &f32 = m.at_2d::<f32>(r, 2)?;
            let h: &f32 = m.at_2d::<f32>(r, 3)?;
            let sc: &f32 = m.at_2d::<f32>(r, 4)?;
            
            let score = *sc as f64;

            let confs = m.row(r)?.col_range( &core::Range::new(5, m.row(r)?.cols())?)?;
            
            let c = (confs * score).into_result()?.to_mat()?;
            
            let mut min_val = Some(0f64);
            let mut max_val = Some(0f64);

            let mut min_loc  = Some(core::Point::default());
            let mut max_loc  = Some(core::Point::default());
            let mut idk = core::no_array();

            // find predicted class with highest confidence
            core::min_max_loc(&c, min_val.as_mut(), max_val.as_mut(), min_loc.as_mut(), max_loc.as_mut(), &mut idk)?;
            
            scores.push(max_val.unwrap() as f32 );
            boxes.push( core::Rect{x: ((*cx) - (*w) / 2.0).round() as i32, y: ((*cy) - (*h) / 2.0).round() as i32, width: *w as i32, height: *h as i32} );
            indices.push(r);
            
            class_index_list.push(max_loc.unwrap().x);

        }

    }
    dnn::nms_boxes(&boxes, &scores, conf_thresh, nms_thresh, &mut indices, 1.0, 0)?;
    let mut indxs : Vec<i32> = Vec::new();
    for i in &indices {
        indxs.push(class_index_list.get(i as usize)?);
    }

    Ok(DetectionOuput{
        boxes: boxes,
        scores: scores,
        indices: indices,
        class_index_list: indxs
    })

}


fn draw_predictions(img: &mut core::Mat, detection_output: &DetectionOuput) -> opencv::Result<()> {

    let boxes = &detection_output.boxes;
    let scores = &detection_output.scores;
    let indices = &detection_output.indices;
    let class_index_list = &detection_output.class_index_list;

    let l = indices.len();
    println!("Num detections: {l}");
    for i in 0..indices.len() {
        let rect = boxes.get(indices.get(i)? as usize)?;

        let label = "A";

        let color = core::Scalar::all(0.0);

        opencv::imgproc::rectangle(img, rect, color, 1, opencv::imgproc::LINE_8, 0)?;
    }

    opencv::imgcodecs::imwrite("boxes.jpg", img, &core::Vector::default())?;
    Ok(())
}